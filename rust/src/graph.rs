use crate::{
    eval_result::EvalResult,
    image::{
        Image, ImageBlock, ImageBlockQueue, PixelIndex, PixelState, QueuedBlockIndex,
        SubdivisionDir, MAX_DISCRETE_N_THETA,
    },
    interval_set::{DecSignSet, SignSet},
    ops::StaticForm,
    relation::{EvalCache, EvalCacheLevel, Relation, RelationType},
};
use image::{imageops, GrayAlphaImage, LumaA, Rgb, RgbImage};
use inari::{const_interval, interval, Decoration, Interval};
use std::{
    convert::TryFrom,
    error, fmt,
    time::{Duration, Instant},
};

/// A possibly empty rectangular region of the Cartesian plane.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Region(Interval, Interval);

impl Region {
    /// Returns the intersection of the regions.
    fn intersection(&self, rhs: &Self) -> Self {
        Self(self.0.intersection(rhs.0), self.1.intersection(rhs.1))
    }

    /// Returns `true` if the region is empty.
    fn is_empty(&self) -> bool {
        self.0.is_empty() || self.1.is_empty()
    }
}

/// A rectangular region of the Cartesian plane with inexact bounds.
///
/// Conceptually, it is a pair of two rectangular regions: *inner* and *outer*
/// that satisfy `inner ⊆ outer`. The inner region can be empty, while the outer one cannot.
#[derive(Clone, Debug)]
pub struct InexactRegion {
    l: Interval,
    r: Interval,
    b: Interval,
    t: Interval,
}

impl InexactRegion {
    /// Creates a new [`InexactRegion`] with the given bounds.
    pub fn new(l: Interval, r: Interval, b: Interval, t: Interval) -> Self {
        assert!(l.inf() <= r.sup() && b.inf() <= t.sup());
        Self { l, r, b, t }
    }

    /// Returns the height of the region.
    fn height(&self) -> Interval {
        self.t - self.b
    }

    /// Returns the inner region.
    fn inner(&self) -> Region {
        Region(
            {
                let l = self.l.sup();
                let r = self.r.inf();
                if l <= r {
                    interval!(l, r).unwrap()
                } else {
                    Interval::EMPTY
                }
            },
            {
                let b = self.b.sup();
                let t = self.t.inf();
                if b <= t {
                    interval!(b, t).unwrap()
                } else {
                    Interval::EMPTY
                }
            },
        )
    }

    /// Returns the outer region.
    fn outer(&self) -> Region {
        Region(
            interval!(self.l.inf(), self.r.sup()).unwrap(),
            interval!(self.b.inf(), self.t.sup()).unwrap(),
        )
    }

    /// Returns a subset of the outer region.
    ///
    /// It is assumed that the region is obtained from the given block.
    /// When applied to a set of regions/blocks which form a partition of a pixel,
    /// the results form a partition of the outer boundary of the pixel.
    ///
    /// Precondition: the block is a subpixel.
    fn subpixel_outer(&self, blk: ImageBlock) -> Region {
        let mask_x = blk.pixel_align_x() - 1;
        let mask_y = blk.pixel_align_y() - 1;

        let l = if blk.x & mask_x == 0 {
            self.l.inf()
        } else {
            self.l.mid()
        };
        let r = if (blk.x + 1) & mask_x == 0 {
            self.r.sup()
        } else {
            self.r.mid()
        };
        let b = if blk.y & mask_y == 0 {
            self.b.inf()
        } else {
            self.b.mid()
        };
        let t = if (blk.y + 1) & mask_y == 0 {
            self.t.sup()
        } else {
            self.t.mid()
        };
        Region(interval!(l, r).unwrap(), interval!(b, t).unwrap())
    }

    /// Returns the width of the region.
    fn width(&self) -> Interval {
        self.r - self.l
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphingErrorKind {
    BlockIndexOverflow,
    ReachedMemLimit,
    ReachedSubdivisionLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphingError {
    pub kind: GraphingErrorKind,
}

impl fmt::Display for GraphingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            GraphingErrorKind::BlockIndexOverflow => write!(f, "block index overflow"),
            GraphingErrorKind::ReachedMemLimit => write!(f, "reached the memory usage limit"),
            GraphingErrorKind::ReachedSubdivisionLimit => {
                write!(f, "reached the subdivision limit")
            }
        }
    }
}

impl error::Error for GraphingError {}

#[derive(Clone, Debug)]
pub struct GraphingStatistics {
    pub pixels: usize,
    pub pixels_proven: usize,
    pub eval_count: usize,
    pub time_elapsed: Duration,
}

pub struct Graph {
    rel: Relation,
    forms: Vec<StaticForm>,
    relation_type: RelationType,
    im: Image,
    // Queue blocks that will be subdivided instead of the divided blocks to save memory.
    bs_to_subdivide: ImageBlockQueue,
    // Affine transformation from pixel coordinates (px, py) to real coordinates (x, y):
    //
    //   ⎛ x ⎞   ⎛ sx   0  tx ⎞ ⎛ px ⎞
    //   ⎜ y ⎟ = ⎜  0  sy  ty ⎟ ⎜ py ⎟.
    //   ⎝ 1 ⎠   ⎝  0   0   1 ⎠ ⎝  1 ⎠
    sx: Interval,
    sy: Interval,
    tx: Interval,
    ty: Interval,
    stats: GraphingStatistics,
    mem_limit: usize,
}

impl Graph {
    pub fn new(
        rel: Relation,
        region: InexactRegion,
        im_width: u32,
        im_height: u32,
        mem_limit: usize,
    ) -> Self {
        let forms = rel.forms().clone();
        let relation_type = rel.relation_type();
        let mut g = Self {
            rel,
            forms,
            relation_type,
            im: Image::new(im_width, im_height),
            bs_to_subdivide: ImageBlockQueue::new(relation_type == RelationType::Polar),
            sx: region.width() / Self::point_interval(im_width as f64),
            sy: region.height() / Self::point_interval(im_height as f64),
            tx: region.l,
            ty: region.b,
            stats: GraphingStatistics {
                pixels: im_width as usize * im_height as usize,
                pixels_proven: 0,
                eval_count: 0,
                time_elapsed: Duration::new(0, 0),
            },
            mem_limit,
        };
        let k = (im_width.max(im_height) as f64).log2().ceil() as i8;
        if relation_type == RelationType::Polar {
            let bs = [
                const_interval!(f64::NEG_INFINITY, -1.0),
                const_interval!(0.0, 0.0),
                const_interval!(1.0, f64::INFINITY),
            ]
            .iter()
            .map(|&n_theta| ImageBlock::new(0, 0, k, k, n_theta))
            .collect::<Vec<_>>();
            g.set_last_queued_block(&bs[2], 2).unwrap();
            for b in bs {
                g.bs_to_subdivide.push_back(b);
            }
        } else {
            g.bs_to_subdivide
                .push_back(ImageBlock::new(0, 0, k, k, Interval::ENTIRE));
        }
        g
    }

    pub fn get_gray_alpha_image(&self, im: &mut GrayAlphaImage) {
        assert!(im.width() == self.im.width() && im.height() == self.im.height());
        for (src, dst) in self.im.state_iter().copied().zip(im.pixels_mut()) {
            *dst = match src {
                PixelState::True => LumaA([0, 255]),
                PixelState::False => LumaA([0, 0]),
                _ => LumaA([0, 128]),
            }
        }
        imageops::flip_vertical_in_place(im);
    }

    pub fn get_image(&self, im: &mut RgbImage) {
        assert!(im.width() == self.im.width() && im.height() == self.im.height());
        for (src, dst) in self.im.state_iter().copied().zip(im.pixels_mut()) {
            *dst = match src {
                PixelState::True => Rgb([0, 0, 0]),
                PixelState::False => Rgb([255, 255, 255]),
                _ => Rgb([64, 128, 192]),
            }
        }
        imageops::flip_vertical_in_place(im);
    }

    pub fn get_statistics(&self) -> GraphingStatistics {
        GraphingStatistics {
            pixels_proven: self
                .im
                .state_iter()
                .copied()
                .filter(|&s| s != PixelState::Uncertain)
                .count(),
            eval_count: self.rel.eval_count(),
            ..self.stats
        }
    }

    /// Refines the graph for a given amount of time.
    ///
    /// Returns `Ok(true)`/`Ok(false)` if graphing is complete/incomplete after refinement.
    pub fn refine(&mut self, timeout: Duration) -> Result<bool, GraphingError> {
        let now = Instant::now();
        let result = self.refine_impl(timeout, &now);
        self.stats.time_elapsed += now.elapsed();
        result
    }

    fn refine_impl(&mut self, timeout: Duration, now: &Instant) -> Result<bool, GraphingError> {
        let mut sub_bs = vec![];
        let mut incomplete_sub_bs = vec![];
        // Blocks are queued in the Morton order. Thus, the caches should work efficiently.
        let mut cache_eval_on_region = EvalCache::new(EvalCacheLevel::PerAxis);
        let mut cache_eval_on_point = EvalCache::new(EvalCacheLevel::Full);
        while let Some((bi, b)) = self.bs_to_subdivide.pop_front() {
            match b.next_dir {
                SubdivisionDir::NTheta => Self::subdivide_on_n_theta(&mut sub_bs, b),
                SubdivisionDir::XY => self.subdivide_on_xy(&mut sub_bs, b),
            }

            let n_sub_bs = sub_bs.len();
            for (sub_b, is_last_sibling) in sub_bs.drain(..) {
                let complete = if !sub_b.is_subpixel() {
                    self.refine_pixel(
                        sub_b,
                        is_last_sibling,
                        QueuedBlockIndex::try_from(bi).unwrap(),
                        &mut cache_eval_on_region,
                    )
                } else {
                    self.refine_subpixel(
                        sub_b,
                        is_last_sibling,
                        QueuedBlockIndex::try_from(bi).unwrap(),
                        &mut cache_eval_on_region,
                        &mut cache_eval_on_point,
                    )
                };
                if !complete {
                    // We can't queue the block yet because we need to modify `sub_b.next_dir`
                    // after all sub-blocks are processed.
                    self.set_last_queued_block(
                        &sub_b,
                        self.bs_to_subdivide.next_back_index() + incomplete_sub_bs.len(),
                    )?;
                    incomplete_sub_bs.push(sub_b);
                }
            }

            let next_dir = if self.relation_type == RelationType::Polar {
                (if 4 * incomplete_sub_bs.len() <= n_sub_bs {
                    // Subdivide in the same direction again.
                    match b.next_dir {
                        SubdivisionDir::NTheta if b.is_subdivisible_on_n_theta() => {
                            Some(SubdivisionDir::NTheta)
                        }
                        SubdivisionDir::XY if b.is_subdivisible_on_xy() => Some(SubdivisionDir::XY),
                        _ => None,
                    }
                } else {
                    // Subdivide in other direction.
                    match b.next_dir {
                        SubdivisionDir::NTheta if b.is_subdivisible_on_xy() => {
                            Some(SubdivisionDir::XY)
                        }
                        SubdivisionDir::XY if b.is_subdivisible_on_n_theta() => {
                            Some(SubdivisionDir::NTheta)
                        }
                        _ => None,
                    }
                })
                .unwrap_or({
                    // Fallback.
                    if b.is_subdivisible_on_n_theta() {
                        SubdivisionDir::NTheta
                    } else if b.is_subdivisible_on_xy() {
                        SubdivisionDir::XY
                    } else {
                        // TODO: Continue processing remaining blocks.
                        return Err(GraphingError {
                            kind: GraphingErrorKind::ReachedSubdivisionLimit,
                        });
                    }
                })
            } else if b.is_subdivisible_on_xy() {
                SubdivisionDir::XY
            } else {
                // TODO: Continue processing remaining blocks.
                return Err(GraphingError {
                    kind: GraphingErrorKind::ReachedSubdivisionLimit,
                });
            };

            for mut sub_b in incomplete_sub_bs.drain(..) {
                sub_b.next_dir = next_dir;
                self.bs_to_subdivide.push_back(sub_b);
            }

            let mut clear_cache_and_retry = true;
            while self.im.size_in_heap()
                + self.bs_to_subdivide.size_in_heap()
                + cache_eval_on_region.size_in_heap()
                + cache_eval_on_point.size_in_heap()
                > self.mem_limit
            {
                if clear_cache_and_retry {
                    cache_eval_on_region = EvalCache::new(EvalCacheLevel::PerAxis);
                    cache_eval_on_point = EvalCache::new(EvalCacheLevel::Full);
                    clear_cache_and_retry = false;
                } else {
                    return Err(GraphingError {
                        kind: GraphingErrorKind::ReachedMemLimit,
                    });
                }
            }

            if now.elapsed() > timeout {
                break;
            }
        }

        Ok(self.bs_to_subdivide.is_empty())
    }

    fn set_last_queued_block(
        &mut self,
        b: &ImageBlock,
        block_index: usize,
    ) -> Result<(), GraphingError> {
        if let Ok(block_index) = QueuedBlockIndex::try_from(block_index) {
            #[allow(clippy::branches_sharing_code)]
            if b.is_superpixel() {
                let pixel_begin = b.pixel_index();
                let pixel_end = PixelIndex::new(
                    (pixel_begin.x + b.width()).min(self.im.width()),
                    (pixel_begin.y + b.height()).min(self.im.height()),
                );
                for y in pixel_begin.y..pixel_end.y {
                    for x in pixel_begin.x..pixel_end.x {
                        let pixel = PixelIndex::new(x, y);
                        *self.im.last_queued_block_mut(pixel) = block_index;
                    }
                }
            } else {
                let pixel = b.pixel_index();
                *self.im.last_queued_block_mut(pixel) = block_index;
            }
            Ok(())
        } else {
            Err(GraphingError {
                kind: GraphingErrorKind::BlockIndexOverflow,
            })
        }
    }

    /// Refine the block and returns `true` if refinement is complete.
    ///
    /// Precondition: the block must be a pixel or a superpixel.
    fn refine_pixel(
        &mut self,
        b: ImageBlock,
        b_is_last_sibling: bool,
        parent_block_index: QueuedBlockIndex,
        cache: &mut EvalCache,
    ) -> bool {
        let pixel_begin = b.pixel_index();
        let pixel_end = PixelIndex::new(
            (pixel_begin.x + b.width()).min(self.im.width()),
            (pixel_begin.y + b.height()).min(self.im.height()),
        );

        let mut all_true = true;
        'outer: for y in pixel_begin.y..pixel_end.y {
            for x in pixel_begin.x..pixel_end.x {
                let pixel = PixelIndex::new(x, y);
                let state = self.im.state(pixel);
                if state != PixelState::True {
                    all_true = false;
                    break 'outer;
                }
            }
        }
        if all_true {
            // All pixels have already been proven to be true.
            return true;
        }

        let u_up = self.block_to_region_clipped(b).outer();
        let r_u_up = Self::eval_on_region(&mut self.rel, &u_up, b.n_theta, Some(cache));
        let is_true = r_u_up
            .map(|DecSignSet(ss, d)| ss == SignSet::ZERO && d >= Decoration::Def)
            .eval(&self.forms[..]);
        let is_false = !r_u_up
            .map(|DecSignSet(ss, _)| ss.contains(SignSet::ZERO))
            .eval(&self.forms[..]);

        if !(is_true || is_false) {
            return false;
        }

        for y in pixel_begin.y..pixel_end.y {
            for x in pixel_begin.x..pixel_end.x {
                let pixel = PixelIndex::new(x, y);
                let state = self.im.state(pixel);
                assert_ne!(state, PixelState::False);
                if state == PixelState::True {
                    // This pixel has already been proven to be true.
                    continue;
                }

                if is_true {
                    *self.im.state_mut(pixel) = PixelState::True;
                } else if is_false
                    && b_is_last_sibling
                    && self.im.last_queued_block(pixel) == parent_block_index
                {
                    *self.im.state_mut(pixel) = PixelState::False;
                }
            }
        }
        true
    }

    /// Refine the block and returns `true` if refinement is complete.
    ///
    /// Precondition: the block must be a subpixel.
    fn refine_subpixel(
        &mut self,
        b: ImageBlock,
        b_is_last_sibling: bool,
        parent_block_index: QueuedBlockIndex,
        cache_eval_on_region: &mut EvalCache,
        cache_eval_on_point: &mut EvalCache,
    ) -> bool {
        let pixel = b.pixel_index();
        let state = self.im.state(pixel);
        assert_ne!(state, PixelState::False);
        if state == PixelState::True {
            // This pixel has already been proven to be true.
            return true;
        }

        let u_up = self.block_to_region(b).subpixel_outer(b);
        let r_u_up =
            Self::eval_on_region(&mut self.rel, &u_up, b.n_theta, Some(cache_eval_on_region));

        let p_dn = self.block_to_region(b.pixel_block()).inner();
        let inter = u_up.intersection(&p_dn);

        // Save `locally_zero_mask` for later use (see the comment below).
        let locally_zero_mask =
            r_u_up.map(|DecSignSet(ss, d)| ss == SignSet::ZERO && d >= Decoration::Def);
        if locally_zero_mask.eval(&self.forms[..]) && !inter.is_empty() {
            // The relation is true everywhere in the subpixel, and the subpixel certainly overlaps
            // with the pixel. Therefore, the pixel contains a solution.
            *self.im.state_mut(pixel) = PixelState::True;
            return true;
        }
        if !r_u_up
            .map(|DecSignSet(ss, _)| ss.contains(SignSet::ZERO))
            .eval(&self.forms[..])
        {
            // The relation is false everywhere in the subpixel.
            if b_is_last_sibling && self.im.last_queued_block(pixel) == parent_block_index {
                *self.im.state_mut(pixel) = PixelState::False;
            }
            return true;
        }

        if inter.is_empty() {
            return false;
        }

        // Evaluate the relation for some sample points within the inner bounds of the subpixel
        // and try proving existence of a solution in two ways:
        //
        // a. Test if the relation is true for any of the sample points.
        //    This is useful especially for plotting inequalities such as "lcm(x, y) ≤ 1".
        //
        // b. Use the intermediate value theorem.
        //    A note about `locally_zero_mask` (for plotting conjunction):
        //    Suppose we are plotting "y = sin(x) && x ≥ 0".
        //    If the conjunct "x ≥ 0" is true throughout the subpixel
        //    and "y - sin(x)" evaluates to `POS` for a sample point and `NEG` for another,
        //    we can conclude that there is a point within the subpixel where the entire relation holds.
        //    Such observation would not be possible by merely converting the relation to
        //    "|y - sin(x)| + |x ≥ 0 ? 0 : 1| = 0".
        let dac_mask = r_u_up.map(|DecSignSet(_, d)| d >= Decoration::Dac);

        let points = [
            Self::simple_eval_point(&inter),
            (inter.0.inf(), inter.1.inf()), // bottom left
            (inter.0.sup(), inter.1.inf()), // bottom right
            (inter.0.inf(), inter.1.sup()), // top left
            (inter.0.sup(), inter.1.sup()), // top right
        ];

        let mut neg_mask = r_u_up.map(|_| false);
        let mut pos_mask = neg_mask.clone();
        for point in &points {
            let r = Self::eval_on_point(
                &mut self.rel,
                point.0,
                point.1,
                b.n_theta,
                Some(cache_eval_on_point),
            );

            // `ss` is nonempty if the decoration is ≥ `Def`, which will be ensured
            // by taking bitand with `dac_mask`.
            neg_mask |= r.map(|DecSignSet(ss, _)| (SignSet::NEG | SignSet::ZERO).contains(ss));
            pos_mask |= r.map(|DecSignSet(ss, _)| (SignSet::POS | SignSet::ZERO).contains(ss));

            if r.map(|DecSignSet(ss, d)| ss == SignSet::ZERO && d >= Decoration::Def)
                .eval(&self.forms[..])
                || (&(&neg_mask & &pos_mask) & &dac_mask)
                    .solution_certainly_exists(&self.forms[..], &locally_zero_mask)
            {
                // Found a solution.
                *self.im.state_mut(pixel) = PixelState::True;
                return true;
            }
        }

        false
    }

    fn eval_on_point(
        rel: &mut Relation,
        x: f64,
        y: f64,
        n_theta: Interval,
        cache: Option<&mut EvalCache>,
    ) -> EvalResult {
        rel.eval(
            Self::point_interval(x),
            Self::point_interval(y),
            n_theta,
            cache,
        )
    }

    fn eval_on_region(
        rel: &mut Relation,
        r: &Region,
        n_theta: Interval,
        cache: Option<&mut EvalCache>,
    ) -> EvalResult {
        rel.eval(r.0, r.1, n_theta, cache)
    }

    /// Returns a point within the given region whose coordinates have as many trailing zeros
    /// as they can in their significand bits.
    /// At such a point, arithmetic expressions are more likely to be evaluated to exact numbers.
    fn simple_eval_point(r: &Region) -> (f64, f64) {
        fn f(x: Interval) -> f64 {
            let a = x.inf();
            let b = x.sup();
            let a_bits = a.to_bits();
            let b_bits = b.to_bits();
            let diff = a_bits ^ b_bits;
            // The number of leading equal bits.
            let n = diff.leading_zeros();
            if n == 64 {
                return a;
            }
            // Set all bits from the MSB through the first differing bit.
            let mask = !0u64 << (64 - n - 1);
            if a <= 0.0 {
                f64::from_bits(a_bits & mask)
            } else {
                f64::from_bits(b_bits & mask)
            }
        }

        (f(r.0), f(r.1))
    }

    /// Returns the region that corresponds to a subpixel block `b`.
    fn block_to_region(&self, b: ImageBlock) -> InexactRegion {
        let pw = b.widthf();
        let ph = b.heightf();
        let px = b.x as f64 * pw;
        let py = b.y as f64 * ph;
        InexactRegion::new(
            Self::point_interval(px).mul_add(self.sx, self.tx),
            Self::point_interval(px + pw).mul_add(self.sx, self.tx),
            Self::point_interval(py).mul_add(self.sy, self.ty),
            Self::point_interval(py + ph).mul_add(self.sy, self.ty),
        )
    }

    /// Returns the region that corresponds to a pixel or superpixel block `b`.
    fn block_to_region_clipped(&self, b: ImageBlock) -> InexactRegion {
        let pw = b.widthf();
        let ph = b.heightf();
        let px = b.x as f64 * pw;
        let py = b.y as f64 * ph;
        InexactRegion::new(
            Self::point_interval(px).mul_add(self.sx, self.tx),
            Self::point_interval((px + pw).min(self.im.width() as f64)).mul_add(self.sx, self.tx),
            Self::point_interval(py).mul_add(self.sy, self.ty),
            Self::point_interval((py + ph).min(self.im.height() as f64)).mul_add(self.sy, self.ty),
        )
    }

    fn point_interval(x: f64) -> Interval {
        interval!(x, x).unwrap()
    }

    /// Subdivides the block both horizontally and vertically and appends the sub-blocks to `sub_bs`.
    fn subdivide_on_xy(&self, sub_bs: &mut Vec<(ImageBlock, bool)>, b: ImageBlock) {
        if b.is_superpixel() {
            let x0 = 2 * b.x;
            let y0 = 2 * b.y;
            let x1 = x0 + 1;
            let y1 = y0 + 1;
            let kx = b.kx - 1;
            let ky = b.ky - 1;
            let b00 = ImageBlock::new(x0, y0, kx, ky, b.n_theta);
            sub_bs.push((b00, true));
            if y1 * b00.height() < self.im.height() {
                sub_bs.push((ImageBlock::new(x0, y1, kx, ky, b.n_theta), true));
            }
            if x1 * b00.width() < self.im.width() {
                sub_bs.push((ImageBlock::new(x1, y0, kx, ky, b.n_theta), true));
            }
            if x1 * b00.width() < self.im.width() && y1 * b00.height() < self.im.height() {
                sub_bs.push((ImageBlock::new(x1, y1, kx, ky, b.n_theta), true));
            }
        } else {
            match self.relation_type {
                RelationType::FunctionOfX => {
                    // Subdivide only horizontally.
                    let x0 = 2 * b.x;
                    let x1 = x0 + 1;
                    let y = b.y;
                    let kx = b.kx - 1;
                    let ky = b.ky;
                    sub_bs.push((ImageBlock::new(x0, y, kx, ky, b.n_theta), false));
                    sub_bs.push((ImageBlock::new(x1, y, kx, ky, b.n_theta), true));
                }
                RelationType::FunctionOfY => {
                    // Subdivide only vertically.
                    let x = b.x;
                    let y0 = 2 * b.y;
                    let y1 = y0 + 1;
                    let kx = b.kx;
                    let ky = b.ky - 1;
                    sub_bs.push((ImageBlock::new(x, y0, kx, ky, b.n_theta), false));
                    sub_bs.push((ImageBlock::new(x, y1, kx, ky, b.n_theta), true));
                }
                _ => {
                    let x0 = 2 * b.x;
                    let y0 = 2 * b.y;
                    let x1 = x0 + 1;
                    let y1 = y0 + 1;
                    let kx = b.kx - 1;
                    let ky = b.ky - 1;
                    sub_bs.push((ImageBlock::new(x0, y0, kx, ky, b.n_theta), false));
                    sub_bs.push((ImageBlock::new(x1, y0, kx, ky, b.n_theta), false));
                    sub_bs.push((ImageBlock::new(x0, y1, kx, ky, b.n_theta), false));
                    sub_bs.push((ImageBlock::new(x1, y1, kx, ky, b.n_theta), true));
                }
            }
        }
    }

    /// Subdivides `b.n_theta` and appends the sub-blocks to `sub_bs`.
    fn subdivide_on_n_theta(sub_bs: &mut Vec<(ImageBlock, bool)>, b: ImageBlock) {
        fn bisect(n: Interval) -> [Option<Interval>; 2] {
            let na = n.inf();
            let nb = n.sup();
            if n.is_singleton() || n.mig() > MAX_DISCRETE_N_THETA {
                [Some(n), None]
            } else if na + 1.0 == nb {
                [
                    Some(interval!(na, na).unwrap()),
                    Some(interval!(nb, nb).unwrap()),
                ]
            } else {
                let mid = if na == f64::NEG_INFINITY {
                    2.0 * nb
                } else if nb == f64::INFINITY {
                    2.0 * na
                } else {
                    (na + nb) / 2.0
                };
                [
                    Some(interval!(na, mid).unwrap()),
                    Some(interval!(mid, nb).unwrap()),
                ]
            }
        }

        // Bisect twice to get four sub-blocks at most.
        let ns = bisect(b.n_theta);
        sub_bs.extend(
            ns.iter()
                .filter_map(|&n| n)
                .flat_map(bisect)
                .flatten()
                .map(|n| (ImageBlock { n_theta: n, ..b }, false)),
        );
        if let Some(last) = sub_bs.last_mut() {
            last.1 = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inexact_region() {
        let u = InexactRegion::new(
            const_interval!(0.33, 0.34),
            const_interval!(0.66, 0.67),
            const_interval!(1.33, 1.34),
            const_interval!(1.66, 1.67),
        );

        assert_eq!(
            u.inner(),
            Region(const_interval!(0.34, 0.66), const_interval!(1.34, 1.66))
        );

        assert_eq!(
            u.outer(),
            Region(const_interval!(0.33, 0.67), const_interval!(1.33, 1.67))
        );

        // The bottom/left sides are pixel boundaries.
        let b = ImageBlock::new(4, 8, -2, -2, Interval::ENTIRE);
        let u_up = u.subpixel_outer(b);
        assert_eq!(u_up.0.inf(), u.l.inf());
        assert_eq!(u_up.0.sup(), u.r.mid());
        assert_eq!(u_up.1.inf(), u.b.inf());
        assert_eq!(u_up.1.sup(), u.t.mid());

        // The top/right sides are pixel boundaries.
        let b = ImageBlock::new(b.x + 3, b.y + 3, -2, -2, Interval::ENTIRE);
        let u_up = u.subpixel_outer(b);
        assert_eq!(u_up.0.inf(), u.l.mid());
        assert_eq!(u_up.0.sup(), u.r.sup());
        assert_eq!(u_up.1.inf(), u.b.mid());
        assert_eq!(u_up.1.sup(), u.t.sup());

        let u = InexactRegion::new(
            const_interval!(0.33, 0.66),
            const_interval!(0.34, 0.67),
            const_interval!(1.33, 1.66),
            const_interval!(1.34, 1.67),
        );

        assert_eq!(u.inner(), Region(Interval::EMPTY, Interval::EMPTY));

        assert_eq!(
            u.outer(),
            Region(const_interval!(0.33, 0.67), const_interval!(1.33, 1.67))
        );
    }
}
