use crate::{
    ast::{BinaryOp, Expr, ExprId, ExprKind, NaryOp, UnaryOp, ValueType, VarSet, UNINIT_EXPR_ID},
    interval_set::Site,
    ops::{
        FormIndex, RelOp, ScalarBinaryOp, ScalarUnaryOp, StaticForm, StaticFormKind, StaticTerm,
        StaticTermKind, StoreIndex, TermIndex,
    },
};
use rug::{Integer, Rational};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    marker::Sized,
    mem::take,
    ops::Deref,
};

/// Matches an [`Expr`] of kind [`ExprKind::Binary`].
macro_rules! binary {
    ($($op:pat)|*, $x:pat, $y:pat) => {
        Expr {
            kind: ExprKind::Binary($($op)|*, box $x, box $y),
            ..
        }
    };
}

/// Matches an [`Expr`] of kind [`ExprKind::Constant`].
macro_rules! constant {
    () => {
        Expr {
            kind: ExprKind::Constant(_),
            ..
        }
    };
    ($a:pat) => {
        Expr {
            kind: ExprKind::Constant(box $a),
            ..
        }
    };
}

/// Matches an [`Expr`] of kind [`ExprKind::Nary`].
macro_rules! nary {
    ($($op:pat)|*, $xs:pat) => {
        Expr {
            kind: ExprKind::Nary($($op)|*, $xs),
            ..
        }
    };
}

/// Matches an [`Expr`] of kind [`ExprKind::Unary`].
macro_rules! unary {
    ($($op:pat)|*, $x:pat) => {
        Expr {
            kind: ExprKind::Unary($($op)|*, box $x),
            ..
        }
    };
}

/// Matches an [`Expr`] of kind [`ExprKind::Var`].
macro_rules! var {
    ($name:pat) => {
        Expr {
            kind: ExprKind::Var($name),
            ..
        }
    };
}

/// A visitor that visits AST nodes in depth-first order.
pub trait Visit<'a>
where
    Self: Sized,
{
    fn visit_expr(&mut self, e: &'a Expr) {
        traverse_expr(self, e);
    }
}

fn traverse_expr<'a, V: Visit<'a>>(v: &mut V, e: &'a Expr) {
    use ExprKind::*;
    match &e.kind {
        Unary(_, x) => v.visit_expr(x),
        Binary(_, x, y) => {
            v.visit_expr(x);
            v.visit_expr(y);
        }
        Nary(_, xs) => {
            for x in xs {
                v.visit_expr(x);
            }
        }
        Pown(x, _) => v.visit_expr(x),
        Rootn(x, _) => v.visit_expr(x),
        List(xs) => {
            for x in xs {
                v.visit_expr(x);
            }
        }
        Constant(_) | Var(_) | Uninit => (),
    };
}

/// A visitor that visits AST nodes in depth-first order and possibly modifies them.
pub trait VisitMut
where
    Self: Sized,
{
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        traverse_expr_mut(self, e);
    }
}

fn traverse_expr_mut<V: VisitMut>(v: &mut V, e: &mut Expr) {
    use ExprKind::*;
    match &mut e.kind {
        Unary(_, x) => v.visit_expr_mut(x),
        Binary(_, x, y) => {
            v.visit_expr_mut(x);
            v.visit_expr_mut(y);
        }
        Nary(_, xs) => {
            for x in xs {
                v.visit_expr_mut(x);
            }
        }
        Pown(x, _) => v.visit_expr_mut(x),
        Rootn(x, _) => v.visit_expr_mut(x),
        List(xs) => {
            for x in xs {
                v.visit_expr_mut(x);
            }
        }
        Constant(_) | Var(_) | Uninit => (),
    };
}

/// A possibly dangling reference to a value.
/// All operations except `from` and `clone` are **unsafe** despite not being marked as so.
struct UnsafeRef<T: Eq + Hash> {
    ptr: *const T,
}

impl<T: Eq + Hash> UnsafeRef<T> {
    fn from(x: &T) -> Self {
        Self { ptr: x as *const T }
    }
}

impl<T: Eq + Hash> Clone for UnsafeRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Eq + Hash> Copy for UnsafeRef<T> {}

impl<T: Eq + Hash> Deref for UnsafeRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T: Eq + Hash> PartialEq for UnsafeRef<T> {
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { (*self.ptr) == (*rhs.ptr) }
    }
}

impl<T: Eq + Hash> Eq for UnsafeRef<T> {}

impl<T: Eq + Hash> Hash for UnsafeRef<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe { (*self.ptr).hash(state) }
    }
}

/// Replaces the names of [`ExprKind::Var`]s that are equal to `params[i]` with `i.to_string()`.
pub struct Parametrize {
    params: Vec<String>,
}

impl Parametrize {
    pub fn new(params: Vec<String>) -> Self {
        Self { params }
    }
}

impl VisitMut for Parametrize {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        traverse_expr_mut(self, e);

        if let var!(x) = e {
            if let Some(i) = self.params.iter().position(|p| p == x) {
                *x = i.to_string();
            }
        }
    }
}

/// Replaces all expressions of the kind [`ExprKind::Var`] with name "0", "1", …
/// with `args[0]`, `args[1]`, …, respectively.
pub struct Substitute {
    args: Vec<Expr>,
}

impl Substitute {
    pub fn new(args: Vec<Expr>) -> Self {
        Self { args }
    }
}

impl VisitMut for Substitute {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        traverse_expr_mut(self, e);

        if let var!(x) = e {
            if let Ok(i) = x.parse::<usize>() {
                *e = self.args.get(i).unwrap().clone()
            }
        }
    }
}

pub struct ReplaceAll<Rule>
where
    Rule: Fn(&Expr) -> Option<Expr>,
{
    pub modified: bool,
    rule: Rule,
}

impl<Rule> ReplaceAll<Rule>
where
    Rule: Fn(&Expr) -> Option<Expr>,
{
    pub fn new(rule: Rule) -> Self {
        Self {
            modified: false,
            rule,
        }
    }
}

impl<Rule> VisitMut for ReplaceAll<Rule>
where
    Rule: Fn(&Expr) -> Option<Expr>,
{
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        traverse_expr_mut(self, e);

        if let Some(replacement) = (self.rule)(e) {
            *e = replacement;
            self.modified = true;
        }
    }
}

/// Replaces arithmetic expressions with equivalent ones consists of only
/// [`BinaryOp::Pow`], [`NaryOp::Plus`] and [`NaryOp::Times`].
///
/// It also does some ad-hoc transformations, which are mainly for demonstrational purposes.
pub struct PreTransform;

impl VisitMut for PreTransform {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        use {BinaryOp::*, NaryOp::*, UnaryOp::*};
        traverse_expr_mut(self, e);

        match e {
            unary!(Neg, x) => {
                // (Neg x) → (Times -1 x)
                *e = Expr::nary(Times, vec![Expr::minus_one(), take(x)]);
            }
            unary!(Sqrt, x) => {
                // (Sqrt x) → (Pow x 1/2)
                *e = Expr::binary(Pow, box take(x), box Expr::one_half());
            }
            binary!(Add, x, y) => {
                // (Add x y) → (Plus x y)
                *e = Expr::nary(Plus, vec![take(x), take(y)]);
            }
            binary!(Div, unary!(Sin, x), y) if x == y => {
                // Ad-hoc.
                // (Div (Sin x) x) → (Sinc (UndefAt0 x))
                *e = Expr::unary(Sinc, box Expr::unary(UndefAt0, box take(x)));
            }
            binary!(Div, x, unary!(Sin, y)) if y == x => {
                // Ad-hoc.
                // (Div x (Sin x)) → (Pow (Sinc (UndefAt0 x)) -1)
                *e = Expr::binary(
                    Pow,
                    box Expr::unary(Sinc, box Expr::unary(UndefAt0, box take(x))),
                    box Expr::minus_one(),
                );
            }
            binary!(Div, x, y) => {
                // (Div x y) → (Times x (Pow y -1))
                *e = Expr::nary(
                    Times,
                    vec![
                        take(x),
                        Expr::binary(Pow, box take(y), box Expr::minus_one()),
                    ],
                );
            }
            binary!(Mul, x, y) => {
                // (Mul x y) → (Times x y)
                *e = Expr::nary(Times, vec![take(x), take(y)]);
            }
            binary!(Sub, x, y) => {
                // (Sub x y) → (Plus x (Times -1 y))
                *e = Expr::nary(
                    Plus,
                    vec![take(x), Expr::nary(Times, vec![Expr::minus_one(), take(y)])],
                );
            }
            _ => (),
        }
    }
}

// TODO: Update doc comment
/// Flattens out terms nested inside [`NaryOp::Plus`] and [`NaryOp::Times`].
/// If an expression contains only one term, it is replaced by the term.
#[derive(Default)]
pub struct Flatten {
    pub modified: bool,
}

impl VisitMut for Flatten {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        use NaryOp::*;
        traverse_expr_mut(self, e);

        if let nary!(op @ (Plus | Times), xs) = e {
            match &mut xs[..] {
                [] => {
                    *e = match op {
                        Plus => Expr::zero(),
                        Times => Expr::one(),
                    };
                    self.modified = true;
                }
                [x] => {
                    *e = take(x);
                    self.modified = true;
                }
                _ => {
                    *xs = xs.drain(..).fold(vec![], |mut acc, x| {
                        match x {
                            nary!(opx, mut xs) if opx == *op => {
                                acc.append(&mut xs);
                                self.modified = true;
                            }
                            _ => acc.push(x),
                        }
                        acc
                    });
                }
            }
        }
    }
}

/// Sorts terms in [`NaryOp::Plus`] and [`NaryOp::Times`] to bring similar ones together.
/// Terms of kind [`ExprKind::Constant`] are moved to the beginning.
// TODO: Move integer constants first to merge exponentiation.
#[derive(Default)]
pub struct SortTerms {
    pub modified: bool,
}

fn cmp_terms(x: &Expr, y: &Expr) -> Ordering {
    use {BinaryOp::*, NaryOp::*};
    match (x, y) {
        (constant!(x), constant!(y)) => {
            let x = x.0.iter().next().unwrap().x.inf();
            let y = y.0.iter().next().unwrap().x.inf();
            x.partial_cmp(&y).unwrap()
        }
        (constant!(_), _) => Ordering::Less,
        (_, constant!(_)) => Ordering::Greater,
        (binary!(Pow, x1, x2), binary!(Pow, y1, y2)) => {
            cmp_terms(x1, y1).then_with(|| cmp_terms(x2, y2))
        }
        (binary!(Pow, x1, x2), _) => cmp_terms(x1, y).then_with(|| cmp_terms(x2, &Expr::one())),
        (_, binary!(Pow, y1, y2)) => cmp_terms(x, y1).then_with(|| cmp_terms(&Expr::one(), y2)),
        (var!(x), var!(y)) => x.cmp(y),
        (var!(_), _) => Ordering::Less,
        (_, var!(_)) => Ordering::Greater,
        (nary!(Times, xs), nary!(Times, ys)) => (|| {
            for (x, y) in xs.iter().rev().zip(ys.iter().rev()) {
                let ord = cmp_terms(x, y);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            xs.len().cmp(&ys.len())
        })(),
        _ => Ordering::Equal,
    }
}

impl VisitMut for SortTerms {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        use NaryOp::*;
        traverse_expr_mut(self, e);

        if let nary!(Plus | Times, xs) = e {
            if xs
                .windows(2)
                .any(|xs| cmp_terms(&xs[0], &xs[1]) == Ordering::Greater)
            {
                // (op x y) /; y ≺ x → (op y x)
                xs.sort_by(|x, y| cmp_terms(x, y));
                self.modified = true;
            }
        }
    }
}

/// Selectively merges consecutive expressions.
fn transform_vec<F>(xs: &mut Vec<Expr>, f: F) -> bool
where
    F: Fn(&mut Expr, &mut Expr) -> Option<Expr>,
{
    let mut modified = false;
    *xs = xs.drain(..).fold(vec![], |mut acc, mut next| {
        if acc.is_empty() {
            acc.push(next)
        } else {
            let last = acc.last_mut().unwrap();
            if let Some(x) = f(last, &mut next) {
                *last = x;
                modified = true;
            } else {
                acc.push(next);
            }
        }
        acc
    });
    modified
}

fn test_rational<F>(x: &Expr, f: F) -> bool
where
    F: Fn(&Rational) -> bool,
{
    match x {
        constant!((_, Some(x))) => f(&x),
        _ => panic!("`x` is not a constant node"),
    }
}

fn test_rationals<F>(x: &Expr, y: &Expr, f: F) -> bool
where
    F: Fn(&Rational, &Rational) -> bool,
{
    match (x, y) {
        (constant!((_, Some(x))), constant!((_, Some(y)))) => f(&x, &y),
        _ => panic!("`x` or `y` is not a constant node"),
    }
}

/// Transforms expressions into simpler/normalized forms.
#[derive(Default)]
pub struct Transform {
    pub modified: bool,
}

impl VisitMut for Transform {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        use {BinaryOp::*, NaryOp::*, UnaryOp::*};
        traverse_expr_mut(self, e);

        match e {
            binary!(Pow, x, constant!(a)) => {
                match a.0.to_f64() {
                    Some(a) if a == 1.0 => {
                        // (Pow x 1) → x
                        *e = take(x);
                        self.modified = true;
                    }
                    _ => (),
                }
            }
            nary!(Plus, xs) => {
                let len = xs.len();

                // Drop zeros.
                xs.retain(|x| !matches!(x, constant!(a) if a.0.to_f64() == Some(0.0)));

                transform_vec(xs, |x, y| {
                    if x == y {
                        // x + x → 2 x
                        return Some(Expr::nary(Times, vec![Expr::two(), take(x)]));
                    }
                    if let nary!(Times, ys) = y {
                        match &mut ys[..] {
                            [y1 @ constant!(), y2] if x == y2 => {
                                // x + a x → (1 + a) x
                                return Some(Expr::nary(
                                    Times,
                                    vec![Expr::nary(Plus, vec![Expr::one(), take(y1)]), take(x)],
                                ));
                            }
                            _ => (),
                        }
                    }
                    if let (nary!(Times, xs), nary!(Times, ys)) = (x, y) {
                        match (&mut xs[..], &mut ys[..]) {
                            ([x1 @ constant!(), x2s @ ..], [y1 @ constant!(), y2s @ ..])
                                if x2s == y2s =>
                            {
                                // a x… + b x… → (a + b) x…
                                let mut v = vec![Expr::nary(Plus, vec![take(x1), take(y1)])];
                                v.extend(xs.drain(1..));
                                return Some(Expr::nary(Times, v));
                            }
                            (x1s, [y1 @ constant!(), y2s @ ..]) if x1s == y2s => {
                                // x… + a x… → (1 + a) x…
                                let mut v = vec![Expr::nary(Plus, vec![Expr::one(), take(y1)])];
                                v.append(xs);
                                return Some(Expr::nary(Times, v));
                            }
                            _ => (),
                        }
                    }
                    None
                });

                self.modified = xs.len() < len;
            }
            nary!(Times, xs) => {
                // Don't replace 0 x with 0 as that alters the domain of the expression.

                let len = xs.len();

                // Drop ones.
                xs.retain(|x| !matches!(x, constant!(a) if a.0.to_f64() == Some(1.0)));

                transform_vec(xs, |x, y| {
                    // Be careful not to alter the domain of the expression.
                    match (x, y) {
                        (
                            binary!(Pow, x1, x2 @ constant!()),
                            binary!(Pow, y1, y2 @ constant!()),
                        ) if x1 == y1
                            && test_rationals(x2, y2, |a, b| {
                                *a.denom() == 1
                                    && *b.denom() == 1
                                    && (*a < 0 && *b < 0 || *a >= 0 && *b >= 0)
                            }) =>
                        {
                            // x^a x^b /. a, b ∈ ℤ ∧ (a, b < 0 ∨ a, b ≥ 0) → x^(a + b)
                            Some(Expr::binary(
                                Pow,
                                box take(x1),
                                box Expr::nary(Plus, vec![take(x2), take(y2)]),
                            ))
                        }
                        (x, binary!(Pow, y1, y2 @ constant!()))
                            if x == y1 && test_rational(y2, |a| *a.denom() == 1 && *a >= 0) =>
                        {
                            // x x^a /. a ∈ ℤ ∧ a ≥ 0 → x^(1 + a)
                            Some(Expr::binary(
                                Pow,
                                box take(x),
                                box Expr::nary(Plus, vec![Expr::one(), take(y2)]),
                            ))
                        }
                        (x, y) if x == y => {
                            // x x → x^2
                            Some(Expr::binary(Pow, box take(x), box Expr::two()))
                        }
                        _ => None,
                    }
                });

                self.modified = xs.len() < len;
            }
            unary!(Not, x) => {
                match x {
                    binary!(op @ (Eq | Ge | Gt | Le | Lt | Neq | Nge | Ngt | Nle | Nlt), x1, x2) => {
                        // (Not (op x1 x2)) → (!op x1 x2)
                        let neg_op = match op {
                            Eq => Neq,
                            Ge => Nge,
                            Gt => Ngt,
                            Le => Nle,
                            Lt => Nlt,
                            Neq => Eq,
                            Nge => Ge,
                            Ngt => Gt,
                            Nle => Le,
                            Nlt => Lt,
                            _ => unreachable!(),
                        };
                        *e = Expr::binary(neg_op, box take(x1), box take(x2));
                        self.modified = true;
                    }
                    binary!(And, x1, x2) => {
                        // (And (x1 x2)) → (Or (Not x1) (Not x2))
                        *e = Expr::binary(
                            Or,
                            box Expr::unary(Not, box take(x1)),
                            box Expr::unary(Not, box take(x2)),
                        );
                        self.modified = true;
                    }
                    binary!(Or, x1, x2) => {
                        // (Or (x1 x2)) → (And (Not x1) (Not x2))
                        *e = Expr::binary(
                            And,
                            box Expr::unary(Not, box take(x1)),
                            box Expr::unary(Not, box take(x2)),
                        );
                        self.modified = true;
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    }
}

/// Performs constant folding.
#[derive(Default)]
pub struct FoldConstant {
    pub modified: bool,
}

impl VisitMut for FoldConstant {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        use {BinaryOp::*, NaryOp::*};
        traverse_expr_mut(self, e);

        match e {
            constant!() => (),
            nary!(op @ (Plus | Times), xs) => {
                if let [_, constant!(), ..] = &mut xs[..] {
                    let bin_op = match op {
                        Plus => Add,
                        Times => Mul,
                    };
                    self.modified = transform_vec(xs, |x, y| {
                        if let (x @ constant!(), y @ constant!()) = (x, y) {
                            let e = Expr::binary(bin_op, box take(x), box take(y));
                            let (a, ar) = e.eval().unwrap();
                            Some(Expr::constant(a, ar))
                        } else {
                            None
                        }
                    });
                }
            }
            _ => {
                if let Some((x, xr)) = e.eval() {
                    // Only fold constants which evaluate to the empty or a single interval
                    // since the branch cut tracking is not possible with the AST.
                    if x.len() <= 1 {
                        *e = Expr::constant(x, xr);
                        self.modified = true;
                    }
                }
            }
        }
    }
}

pub struct UpdatePolarPeriod;

impl VisitMut for UpdatePolarPeriod {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        use {BinaryOp::*, NaryOp::*, UnaryOp::*};
        traverse_expr_mut(self, e);

        match e {
            constant!(_) => e.polar_period = Some(0.into()),
            var!(name) if name != "theta" && name != "θ" => e.polar_period = Some(0.into()),
            unary!(_, x) if x.polar_period.is_some() => {
                e.polar_period = x.polar_period.clone();
            }
            binary!(_, x, y) if x.polar_period.is_some() && y.polar_period.is_some() => {
                let xp = x.polar_period.as_ref().unwrap();
                let yp = y.polar_period.as_ref().unwrap();
                e.polar_period = Some(if *xp == 0 {
                    yp.clone()
                } else if *yp == 0 {
                    xp.clone()
                } else {
                    xp.lcm_ref(yp).into()
                });
            }
            nary!(_, xs) if xs.iter().all(|x| x.polar_period.is_some()) => {
                e.polar_period = Some(xs.iter().fold(Integer::from(0), |mut xp, y| {
                    let yp = y.polar_period.as_ref().unwrap();
                    if xp == 0 {
                        yp.clone()
                    } else if *yp == 0 {
                        xp
                    } else {
                        xp.lcm_mut(yp);
                        xp
                    }
                }));
            }
            unary!(Cos | Sin | Tan, x) => match x {
                var!(name) if name == "theta" || name == "θ" => {
                    // sin(θ)
                    e.polar_period = Some(1.into());
                }
                nary!(Plus, xs) => match &xs[..] {
                    [constant!(), var!(name)] if name == "theta" || name == "θ" => {
                        // sin(b + θ)
                        e.polar_period = Some(1.into());
                    }
                    [constant!(), binary!(Mul, constant!(a), var!(name))]
                        if name == "theta" || name == "θ" =>
                    {
                        // sin(b + a θ)
                        if let Some(a) = &a.1 {
                            e.polar_period = Some(a.denom().clone())
                        }
                    }
                    _ => (),
                },
                nary!(Times, xs) => match &xs[..] {
                    [constant!(a), var!(name)] if name == "theta" || name == "θ" => {
                        // sin(a θ)
                        if let Some(a) = &a.1 {
                            e.polar_period = Some(a.denom().clone())
                        }
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        }
    }
}

// TODO: doc comment
pub struct SubDivTransform;

impl VisitMut for SubDivTransform {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        use {BinaryOp::*, NaryOp::*, UnaryOp::*};
        traverse_expr_mut(self, e);

        match e {
            nary!(Plus, xs) => {
                let (lhs, rhs) =
                    xs.drain(..)
                        .fold((vec![], vec![]), |(mut lhs, mut rhs), mut e| {
                            match e {
                                nary!(Times, ref mut xs) => match &xs[..] {
                                    [constant!((a, _)), ..] if a.to_f64() == Some(-1.0) => {
                                        rhs.push(Expr::nary(Times, xs.drain(1..).collect()));
                                    }
                                    _ => lhs.push(e),
                                },
                                _ => {
                                    lhs.push(e);
                                }
                            }
                            (lhs, rhs)
                        });

                *e = if lhs.is_empty() {
                    Expr::unary(Neg, box Expr::nary(Plus, rhs))
                } else if rhs.is_empty() {
                    Expr::nary(Plus, lhs)
                } else {
                    Expr::binary(Sub, box Expr::nary(Plus, lhs), box Expr::nary(Plus, rhs))
                };
            }
            nary!(Times, xs) => {
                let (num, den) = xs
                    .drain(..)
                    .fold((vec![], vec![]), |(mut num, mut den), e| {
                        match e {
                            binary!(Pow, x, y @ constant!()) if test_rational(&y, |x| *x < 0.0) => {
                                if let constant!((yi, Some(yr))) = y {
                                    let factor = Expr::binary(
                                        Pow,
                                        box x,
                                        box Expr::constant(-&yi, Some((-&yr).into())),
                                    );
                                    den.push(factor)
                                } else {
                                    panic!()
                                }
                            }
                            _ => {
                                num.push(e);
                            }
                        }
                        (num, den)
                    });

                *e = if num.is_empty() {
                    Expr::unary(Recip, box Expr::nary(Times, den))
                } else if den.is_empty() {
                    Expr::nary(Times, num)
                } else {
                    Expr::binary(Div, box Expr::nary(Times, num), box Expr::nary(Times, den))
                };
            }
            _ => (),
        }
    }
}

/// Replaces arithmetic expressions with more optimal ones suitable for evaluation.
/// It completely removes [`NaryOp::Plus`] and [`NaryOp::Times`].
pub struct PostTransform;

impl VisitMut for PostTransform {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        use {BinaryOp::*, NaryOp::*, UnaryOp::*};
        traverse_expr_mut(self, e);

        match e {
            binary!(Pow, constant!(a), y) => {
                match a.0.to_f64() {
                    Some(a) if a == 2.0 => {
                        // (Pow 2 x) → (Exp2 x)
                        *e = Expr::unary(Exp2, box take(y));
                    }
                    Some(a) if a == 10.0 => {
                        // (Pow 10 x) → (Exp10 x)
                        *e = Expr::unary(Exp10, box take(y));
                    }
                    _ => (),
                }
            }
            binary!(Pow, x, constant!(a)) => {
                if let Some(a) = &a.1 {
                    if let (Some(n), Some(d)) = (a.numer().to_i32(), a.denom().to_u32()) {
                        let root = match d {
                            1 => take(x),
                            2 => Expr::unary(Sqrt, box take(x)),
                            _ => Expr::rootn(box take(x), d),
                        };
                        *e = match n {
                            -1 => Expr::unary(Recip, box root),
                            0 => Expr::unary(One, box root),
                            1 => root,
                            2 => Expr::unary(Sqr, box root),
                            _ => Expr::pown(box root, n),
                        }
                    }
                }
            }
            nary!(Plus, xs) => {
                let mut it = xs.drain(..);
                // Assuming `e` is flattened.
                let first = it.next().unwrap();
                let second = it.next().unwrap();
                let init = Expr::binary(BinaryOp::Add, box first, box second);
                *e = it.fold(init, |e, x| Expr::binary(BinaryOp::Add, box e, box x))
            }
            nary!(Times, xs) => {
                let mut it = xs.drain(..);
                // Assuming `e` is flattened.
                let first = it.next().unwrap();
                let second = it.next().unwrap();
                let init = match first {
                    constant!(a) if a.0.to_f64() == Some(-1.0) => Expr::unary(Neg, box second),
                    _ => Expr::binary(BinaryOp::Mul, box first, box second),
                };
                *e = it.fold(init, |e, x| Expr::binary(BinaryOp::Mul, box e, box x))
            }
            _ => (),
        }
    }
}

/// Updates metadata of terms and formulas.
pub struct UpdateMetadata;

impl VisitMut for UpdateMetadata {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        traverse_expr_mut(self, e);
        e.update_metadata();
    }
}

type SiteMap = HashMap<ExprId, Site>;
type UnsafeExprRef = UnsafeRef<Expr>;

/// Assigns [`ExprId`]s to unique expressions in topological order.
pub struct AssignId {
    next_id: ExprId,
    next_site: u8,
    site_map: SiteMap,
    exprs: Vec<UnsafeExprRef>,
    visited: HashSet<UnsafeExprRef>,
}

impl AssignId {
    pub fn new() -> Self {
        AssignId {
            next_id: 0,
            next_site: 0,
            site_map: HashMap::new(),
            exprs: vec![],
            visited: HashSet::new(),
        }
    }

    /// Returns `true` if the expression can perform branch cut on evaluation.
    fn term_can_perform_cut(kind: &ExprKind) -> bool {
        use {BinaryOp::*, ExprKind::*, UnaryOp::*};
        matches!(
            kind,
            Unary(Ceil | Digamma | Floor | Gamma | Recip | Tan, _)
                | Binary(
                    Atan2 | Div | Gcd | Lcm | Log | Mod | Pow | RankedMax | RankedMin,
                    _,
                    _
                )
                | Pown(_, _)
        )
    }
}

impl VisitMut for AssignId {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        traverse_expr_mut(self, e);

        match self.visited.get(&UnsafeExprRef::from(e)) {
            Some(visited) => {
                let id = visited.id;
                e.id = id;

                if !self.site_map.contains_key(&id)
                    && Self::term_can_perform_cut(&e.kind)
                    && self.next_site <= Site::MAX
                {
                    self.site_map.insert(id, Site::new(self.next_site));
                    self.next_site += 1;
                }
            }
            _ => {
                assert!(self.next_id != UNINIT_EXPR_ID);
                e.id = self.next_id;
                self.next_id += 1;
                let r = UnsafeExprRef::from(e);
                self.exprs.push(r);
                self.visited.insert(r);
            }
        }
    }
}

/// Collects [`StaticTerm`]s and [`StaticForm`]s in ascending order of the IDs.
pub struct CollectStatic {
    pub terms: Vec<StaticTerm>,
    pub forms: Vec<StaticForm>,
    site_map: SiteMap,
    exprs: Vec<UnsafeExprRef>,
    term_index: HashMap<ExprId, TermIndex>,
    form_index: HashMap<ExprId, FormIndex>,
    next_scalar_store_index: u32,
}

impl CollectStatic {
    pub fn new(v: AssignId) -> Self {
        let mut slf = Self {
            terms: vec![],
            forms: vec![],
            site_map: v.site_map,
            exprs: v.exprs,
            term_index: HashMap::new(),
            form_index: HashMap::new(),
            next_scalar_store_index: 0,
        };
        slf.collect_terms();
        slf.collect_atomic_forms();
        slf.collect_non_atomic_forms();
        slf
    }

    pub fn n_scalar_terms(&self) -> usize {
        self.exprs
            .iter()
            .filter(|t| t.ty == ValueType::Scalar)
            .count()
    }

    fn collect_terms(&mut self) {
        use {BinaryOp::*, ExprKind::*, UnaryOp::*};
        for t in self.exprs.iter().map(|t| &*t) {
            let k = match &t.kind {
                Constant(x) => Some(StaticTermKind::Constant(box x.0.clone())),
                Var(x) if x == "x" => Some(StaticTermKind::X),
                Var(x) if x == "y" => Some(StaticTermKind::Y),
                Var(x) if x == "<n-theta>" => Some(StaticTermKind::NTheta),
                Unary(op, x) => match op {
                    Abs => Some(ScalarUnaryOp::Abs),
                    Acos => Some(ScalarUnaryOp::Acos),
                    Acosh => Some(ScalarUnaryOp::Acosh),
                    AiryAi => Some(ScalarUnaryOp::AiryAi),
                    AiryAiPrime => Some(ScalarUnaryOp::AiryAiPrime),
                    AiryBi => Some(ScalarUnaryOp::AiryBi),
                    AiryBiPrime => Some(ScalarUnaryOp::AiryBiPrime),
                    Asin => Some(ScalarUnaryOp::Asin),
                    Asinh => Some(ScalarUnaryOp::Asinh),
                    Atan => Some(ScalarUnaryOp::Atan),
                    Atanh => Some(ScalarUnaryOp::Atanh),
                    Ceil => Some(ScalarUnaryOp::Ceil),
                    Chi => Some(ScalarUnaryOp::Chi),
                    Ci => Some(ScalarUnaryOp::Ci),
                    Cos => Some(ScalarUnaryOp::Cos),
                    Cosh => Some(ScalarUnaryOp::Cosh),
                    Digamma => Some(ScalarUnaryOp::Digamma),
                    Ei => Some(ScalarUnaryOp::Ei),
                    EllipticE => Some(ScalarUnaryOp::EllipticE),
                    EllipticK => Some(ScalarUnaryOp::EllipticK),
                    Erf => Some(ScalarUnaryOp::Erf),
                    Erfc => Some(ScalarUnaryOp::Erfc),
                    Erfi => Some(ScalarUnaryOp::Erfi),
                    Exp => Some(ScalarUnaryOp::Exp),
                    Exp10 => Some(ScalarUnaryOp::Exp10),
                    Exp2 => Some(ScalarUnaryOp::Exp2),
                    Floor => Some(ScalarUnaryOp::Floor),
                    FresnelC => Some(ScalarUnaryOp::FresnelC),
                    FresnelS => Some(ScalarUnaryOp::FresnelS),
                    Gamma => Some(ScalarUnaryOp::Gamma),
                    Li => Some(ScalarUnaryOp::Li),
                    Ln => Some(ScalarUnaryOp::Ln),
                    Log10 => Some(ScalarUnaryOp::Log10),
                    Neg => Some(ScalarUnaryOp::Neg),
                    One => Some(ScalarUnaryOp::One),
                    Recip => Some(ScalarUnaryOp::Recip),
                    Shi => Some(ScalarUnaryOp::Shi),
                    Si => Some(ScalarUnaryOp::Si),
                    Sin => Some(ScalarUnaryOp::Sin),
                    Sinc => Some(ScalarUnaryOp::Sinc),
                    Sinh => Some(ScalarUnaryOp::Sinh),
                    Sqr => Some(ScalarUnaryOp::Sqr),
                    Sqrt => Some(ScalarUnaryOp::Sqrt),
                    Tan => Some(ScalarUnaryOp::Tan),
                    Tanh => Some(ScalarUnaryOp::Tanh),
                    UndefAt0 => Some(ScalarUnaryOp::UndefAt0),
                    _ => None,
                }
                .map(|op| StaticTermKind::Unary(op, self.ti(x))),
                Binary(op, x, y) => match op {
                    Add => Some(ScalarBinaryOp::Add),
                    Atan2 => Some(ScalarBinaryOp::Atan2),
                    BesselI => Some(ScalarBinaryOp::BesselI),
                    BesselJ => Some(ScalarBinaryOp::BesselJ),
                    BesselK => Some(ScalarBinaryOp::BesselK),
                    BesselY => Some(ScalarBinaryOp::BesselY),
                    Div => Some(ScalarBinaryOp::Div),
                    GammaInc => Some(ScalarBinaryOp::GammaInc),
                    Gcd => Some(ScalarBinaryOp::Gcd),
                    Lcm => Some(ScalarBinaryOp::Lcm),
                    Log => Some(ScalarBinaryOp::Log),
                    Max => Some(ScalarBinaryOp::Max),
                    Min => Some(ScalarBinaryOp::Min),
                    Mod => Some(ScalarBinaryOp::Mod),
                    Mul => Some(ScalarBinaryOp::Mul),
                    Pow => Some(ScalarBinaryOp::Pow),
                    RankedMax => Some(ScalarBinaryOp::RankedMax),
                    RankedMin => Some(ScalarBinaryOp::RankedMin),
                    Sub => Some(ScalarBinaryOp::Sub),
                    _ => None,
                }
                .map(|op| StaticTermKind::Binary(op, self.ti(x), self.ti(y))),
                Pown(x, n) => Some(StaticTermKind::Pown(self.ti(x), *n)),
                Rootn(x, n) => Some(StaticTermKind::Rootn(self.ti(x), *n)),
                List(xs) => Some(StaticTermKind::List(
                    box xs.iter().map(|x| self.ti(x)).collect(),
                )),
                Nary(_, _) | Var(_) | Uninit => panic!(),
            };
            if let Some(k) = k {
                self.term_index.insert(t.id, self.terms.len() as TermIndex);
                let store_index = match &t.kind {
                    List(_) => StoreIndex::new(0), // List values are not stored.
                    _ => {
                        let i = self.next_scalar_store_index;
                        self.next_scalar_store_index += 1;
                        StoreIndex::new(i)
                    }
                };
                self.terms.push(StaticTerm {
                    site: self.site_map.get(&t.id).copied(),
                    kind: k,
                    vars: t.vars,
                    store_index,
                })
            }
        }
    }

    fn collect_atomic_forms(&mut self) {
        use {BinaryOp::*, ExprKind::*};
        for t in self.exprs.iter().map(|t| &*t) {
            let k = match &t.kind {
                Binary(op, x, y) => match op {
                    Eq => Some(RelOp::Eq),
                    Ge => Some(RelOp::Ge),
                    Gt => Some(RelOp::Gt),
                    Le => Some(RelOp::Le),
                    Lt => Some(RelOp::Lt),
                    Neq => Some(RelOp::Neq),
                    Nge => Some(RelOp::Nge),
                    Ngt => Some(RelOp::Ngt),
                    Nle => Some(RelOp::Nle),
                    Nlt => Some(RelOp::Nlt),
                    _ => None,
                }
                .map(|op| StaticFormKind::Atomic(op, self.ti(x), self.ti(y))),
                _ => None,
            };
            if let Some(k) = k {
                self.form_index.insert(t.id, self.forms.len() as FormIndex);
                self.forms.push(StaticForm { kind: k })
            }
        }
    }

    fn collect_non_atomic_forms(&mut self) {
        use {BinaryOp::*, ExprKind::*};
        for t in self.exprs.iter().map(|t| &*t) {
            let k = match &t.kind {
                Binary(And, x, y) => Some(StaticFormKind::And(self.fi(x), self.fi(y))),
                Binary(Or, x, y) => Some(StaticFormKind::Or(self.fi(x), self.fi(y))),
                _ => None,
            };
            if let Some(k) = k {
                self.form_index.insert(t.id, self.forms.len() as FormIndex);
                self.forms.push(StaticForm { kind: k })
            }
        }
    }

    fn ti(&self, e: &Expr) -> TermIndex {
        self.term_index[&e.id]
    }

    fn fi(&self, e: &Expr) -> FormIndex {
        self.form_index[&e.id]
    }
}

/// Collects the store indices of maximal scalar sub-expressions that contain exactly one free variable.
/// Expressions of the kind [`ExprKind::Var`] are excluded from collection.
pub struct FindMaximalScalarTerms {
    mx: Vec<StoreIndex>,
    my: Vec<StoreIndex>,
    terms: Vec<StaticTerm>,
    term_index: HashMap<ExprId, TermIndex>,
}

impl FindMaximalScalarTerms {
    pub fn new(collector: CollectStatic) -> Self {
        Self {
            mx: vec![],
            my: vec![],
            terms: collector.terms,
            term_index: collector.term_index,
        }
    }

    pub fn mx_my(mut self) -> (Vec<StoreIndex>, Vec<StoreIndex>) {
        self.mx.sort_unstable();
        self.mx.dedup();
        self.my.sort_unstable();
        self.my.dedup();
        (self.mx, self.my)
    }
}

impl<'a> Visit<'a> for FindMaximalScalarTerms {
    fn visit_expr(&mut self, e: &'a Expr) {
        match e.vars {
            VarSet::EMPTY => {
                // Stop traversal.
            }
            VarSet::X if e.ty == ValueType::Scalar => {
                if !matches!(e.kind, ExprKind::Var(_)) {
                    self.mx
                        .push(self.terms[self.term_index[&e.id] as usize].store_index);
                }
                // Stop traversal.
            }
            VarSet::Y if e.ty == ValueType::Scalar => {
                if !matches!(e.kind, ExprKind::Var(_)) {
                    self.my
                        .push(self.terms[self.term_index[&e.id] as usize].store_index);
                }
                // Stop traversal.
            }
            _ => traverse_expr(self, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{context::Context, parse::parse_expr};
    use rug::Integer;

    #[test]
    fn pre_transform() {
        fn test(input: &str, expected: &str) {
            let mut f = parse_expr(input, Context::builtin_context()).unwrap();
            PreTransform.visit_expr_mut(&mut f);
            assert_eq!(format!("{}", f.dump_structure()), expected);
        }

        test("-x", "(Times -1 x)");
        test("sqrt(x)", "(Pow x 0.5)");
        test("x - y", "(Plus x (Times -1 y))");
        test("x/y", "(Times x (Pow y -1))");
        test("sin(x)/x", "(Sinc (UndefAt0 x))");
        test("x/sin(x)", "(Pow (Sinc (UndefAt0 x)) -1)");
        test("x y", "(Times x y)");
        test("x - y", "(Plus x (Times -1 y))");
    }

    #[test]
    fn flatten() {
        fn test(input: &str, expected: &str) {
            let mut e = parse_expr(input, Context::builtin_context()).unwrap();
            PreTransform.visit_expr_mut(&mut e);
            Transform::default().visit_expr_mut(&mut e);
            let input = format!("{}", e.dump_structure());
            let mut v = Flatten::default();
            v.visit_expr_mut(&mut e);
            let output = format!("{}", e.dump_structure());
            assert_eq!(format!("{}", e.dump_structure()), expected);
            assert_eq!(v.modified, input != output);
        }

        test("x + y + z", "(Plus x y z)");
        test("(x + y) + z", "(Plus x y z)");
        test("x + (y + z)", "(Plus x y z)");
        test("x y z", "(Times x y z)");
        test("(x y) z", "(Times x y z)");
        test("x (y z)", "(Times x y z)");
        test("0 + 0", "0");
        test("1*1", "1");
    }

    #[test]
    fn sort_terms() {
        fn test(input: &str, expected: &str) {
            let mut e = parse_expr(input, Context::builtin_context()).unwrap();
            PreTransform.visit_expr_mut(&mut e);
            FoldConstant::default().visit_expr_mut(&mut e);
            Flatten::default().visit_expr_mut(&mut e);
            FoldConstant::default().visit_expr_mut(&mut e);
            let input = format!("{}", e.dump_structure());
            let mut v = SortTerms::default();
            v.visit_expr_mut(&mut e);
            let output = format!("{}", e.dump_structure());
            assert_eq!(output, expected);
            assert_eq!(v.modified, input != output);
        }

        test("1 + x", "(Plus 1 x)");
        test("x + 1", "(Plus 1 x)");
        test("x + 2x", "(Plus x (Times 2 x))");
        test("2x + x", "(Plus x (Times 2 x))");
        test("2 x", "(Times 2 x)");
        test("x 2", "(Times 2 x)");
        test("x x^2", "(Times x (Pow x 2))");
        test("x^2 x", "(Times x (Pow x 2))");
        test("x x^-1", "(Times (Pow x -1) x)");
        test("x^-1 x", "(Times (Pow x -1) x)");

        test("x y", "(Times x y)");
        test("y x", "(Times x y)");

        test("y z + x y z", "(Plus (Times y z) (Times x y z))");
        test("x y z + y z", "(Plus (Times y z) (Times x y z))");
    }

    #[test]
    fn transform() {
        fn test(input: &str, expected: &str) {
            let mut e = parse_expr(input, Context::builtin_context()).unwrap();
            PreTransform.visit_expr_mut(&mut e);
            Flatten::default().visit_expr_mut(&mut e);
            FoldConstant::default().visit_expr_mut(&mut e);
            Flatten::default().visit_expr_mut(&mut e);
            let input = format!("{}", e.dump_structure());
            let mut v = Transform::default();
            v.visit_expr_mut(&mut e);
            FoldConstant::default().visit_expr_mut(&mut e);
            Flatten::default().visit_expr_mut(&mut e);
            let output = format!("{}", e.dump_structure());
            assert_eq!(output, expected);
            assert_eq!(v.modified, input != output);
        }

        test("0 + x", "x");
        test("x + x", "(Times 2 x)");
        test("x + 2x", "(Times 3 x)");
        test("2x + 3x", "(Times 5 x)");
        test("x y + x y", "(Times 2 x y)");
        test("x y + 2x y", "(Times 3 x y)");
        test("2x y + 3x y", "(Times 5 x y)");

        test("0 sqrt(x)", "(Times 0 (Pow x 0.5))");
        test("1 x", "x");

        test("x x", "(Pow x 2)");
        test("x x^2", "(Pow x 3)");
        test("x^2 x^3", "(Pow x 5)");
        test("x^-2 x^-3", "(Pow x -5)");
        test("x^-2 x^3", "(Times (Pow x -2) (Pow x 3))");
        test("x^2 x^2", "(Pow x 4)");
        test("sqrt(x) sqrt(x)", "(Pow (Pow x 0.5) 2)");

        test("!(x = y)", "(Neq x y)");
        test("!(x ≤ y)", "(Nle x y)");
        test("!(x < y)", "(Nlt x y)");
        test("!(x ≥ y)", "(Nge x y)");
        test("!(x > y)", "(Ngt x y)");
        test("!!(x = y)", "(Eq x y)");
        test("!!(x ≤ y)", "(Le x y)");
        test("!!(x < y)", "(Lt x y)");
        test("!!(x ≥ y)", "(Ge x y)");
        test("!!(x > y)", "(Gt x y)");
        test("!(x = y && y = z)", "(Or (Not (Eq x y)) (Not (Eq y z)))");
        test("!(x = y || y = z)", "(And (Not (Eq x y)) (Not (Eq y z)))");
    }

    #[test]
    fn post_transform() {
        fn test(input: &str, expected: &str) {
            let mut e = parse_expr(input, Context::builtin_context()).unwrap();
            PreTransform.visit_expr_mut(&mut e);
            Flatten::default().visit_expr_mut(&mut e);
            FoldConstant::default().visit_expr_mut(&mut e);
            Flatten::default().visit_expr_mut(&mut e);
            PostTransform.visit_expr_mut(&mut e);
            assert_eq!(format!("{}", e.dump_structure()), expected);
        }

        test("2^x", "(Exp2 x)");
        test("10^x", "(Exp10 x)");
        test("x^-1", "(Recip x)");
        test("x^0", "(One x)");
        test("x^1", "x");
        test("x^2", "(Sqr x)");
        test("x^3", "(Pown x 3)");
        test("x^(1/2)", "(Sqrt x)");
        test("x^(3/2)", "(Pown (Sqrt x) 3)");
        test("x^(-2/3)", "(Pown (Rootn x 3) -2)");
        test("x^(-1/3)", "(Recip (Rootn x 3))");
        test("x^(1/3)", "(Rootn x 3)");
        test("x^(2/3)", "(Sqr (Rootn x 3))");
        test("x + y", "(Add x y)");
        test("x + y + z", "(Add (Add x y) z)");
        test("x y", "(Mul x y)");
        test("x y z", "(Mul (Mul x y) z)");
    }

    #[test]
    fn update_polar_period() {
        fn test(input: &str, expected_period: Option<Integer>) {
            let mut e = parse_expr(input, Context::builtin_context()).unwrap();
            PreTransform.visit_expr_mut(&mut e);
            Flatten::default().visit_expr_mut(&mut e);
            SortTerms::default().visit_expr_mut(&mut e);
            FoldConstant::default().visit_expr_mut(&mut e);
            UpdatePolarPeriod.visit_expr_mut(&mut e);
            assert_eq!(e.polar_period, expected_period);
        }

        test("42", Some(0.into()));
        test("x", Some(0.into()));
        test("y", Some(0.into()));
        test("r", Some(0.into()));
        test("θ", None);
        test("sin(θ)", Some(1.into()));
        test("cos(θ)", Some(1.into()));
        test("tan(θ)", Some(1.into()));
        test("sin(3/5θ)", Some(5.into()));
        test("sqrt(sin(θ))", Some(1.into()));
        test("sin(θ) + θ", None);
        test("min(sin(θ), θ)", None);
        test("r = sin(θ)", Some(1.into()));
        test("sin(3θ/5)", Some(5.into()));
        test("sin(θ/2) + cos(θ/3)", Some(6.into()));
        test("min(sin(θ/2), cos(θ/3))", Some(6.into()));
    }
}
