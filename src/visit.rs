use crate::{ast::*, rel::*};
use std::{
    collections::{HashMap, HashSet},
    marker::Sized,
};

pub trait Visit<'a>
where
    Self: Sized,
{
    fn visit_expr(&mut self, expr: &'a Expr) {
        traverse_expr(self, expr);
    }

    fn visit_rel(&mut self, rel: &'a Rel) {
        traverse_rel(self, rel)
    }
}

fn traverse_expr<'a, V: Visit<'a>>(v: &mut V, expr: &'a Expr) {
    use ExprKind::*;
    match &expr.kind {
        Unary(_, x) => v.visit_expr(x),
        Binary(_, x, y) => {
            v.visit_expr(x);
            v.visit_expr(y);
        }
        Pown(x, _) => v.visit_expr(x),
        _ => (),
    };
}

fn traverse_rel<'a, V: Visit<'a>>(v: &mut V, rel: &'a Rel) {
    use RelKind::*;
    match &rel.kind {
        Equality(_, x, y) => {
            v.visit_expr(x);
            v.visit_expr(y);
        }
        And(x, y) | Or(x, y) => {
            v.visit_rel(x);
            v.visit_rel(y);
        }
    };
}

pub trait VisitMut
where
    Self: Sized,
{
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        traverse_expr_mut(self, expr);
    }

    fn visit_rel_mut(&mut self, rel: &mut Rel) {
        traverse_rel_mut(self, rel);
    }
}

fn traverse_expr_mut<V: VisitMut>(v: &mut V, expr: &mut Expr) {
    use ExprKind::*;
    match &mut expr.kind {
        Unary(_, x) => v.visit_expr_mut(x),
        Binary(_, x, y) => {
            v.visit_expr_mut(x);
            v.visit_expr_mut(y);
        }
        Pown(x, _) => v.visit_expr_mut(x),
        _ => (),
    };
}

fn traverse_rel_mut<V: VisitMut>(v: &mut V, rel: &mut Rel) {
    use RelKind::*;
    match &mut rel.kind {
        Equality(_, x, y) => {
            v.visit_expr_mut(x);
            v.visit_expr_mut(y);
        }
        And(x, y) | Or(x, y) => {
            v.visit_rel_mut(x);
            v.visit_rel_mut(y);
        }
    };
}

type SiteMap = HashMap<ExprId, Option<u8>>;

pub struct Transform;

impl VisitMut for Transform {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        use {BinaryOp::*, ExprKind::*, UnaryOp::*};
        traverse_expr_mut(self, expr);

        match &mut expr.kind {
            Binary(Div, x, y) => {
                match (&x.kind, &y.kind) {
                    // (Div (Sin z) y) => (SinOverX y) if z == y
                    (Unary(Sin, z), _) if z == y => {
                        *expr = Expr::new(Unary(SinOverX, std::mem::take(y)));
                    }
                    // (Div x (Sin z)) => (Recip (SinOverX x)) if z == x
                    (_, Unary(Sin, z)) if z == x => {
                        *expr = Expr::new(Unary(
                            Recip,
                            Box::new(Expr::new(Unary(SinOverX, std::mem::take(x)))),
                        ));
                    }
                    _ => (),
                };
            }
            Pown(x, y) => match y {
                -1 => {
                    *expr = Expr::new(Unary(Recip, std::mem::take(x)));
                }
                // Do not transform x^0 to 1.0 as that could discard the decoration.
                1 => {
                    *expr = *std::mem::take(x);
                }
                2 => {
                    *expr = Expr::new(Unary(Sqr, std::mem::take(x)));
                }
                _ => (),
            },
            _ => (),
        }
    }
}

pub struct FoldConstant;

// Only fold constants which evaluate to an empty or a single interval
// since the sites are not assigned and branch cut tracking is not possible
// at this moment.
impl VisitMut for FoldConstant {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        use ExprKind::*;
        traverse_expr_mut(self, expr);

        match &mut expr.kind {
            Unary(_, x) => {
                if let Constant(_) = &x.kind {
                    let val = expr.evaluate();
                    if val.len() <= 1 {
                        *expr = Expr::new(Constant(box val));
                    }
                }
            }
            Binary(_, x, y) => {
                if let (Constant(_), Constant(_)) = (&x.kind, &y.kind) {
                    let val = expr.evaluate();
                    if val.len() <= 1 {
                        *expr = Expr::new(Constant(box val));
                    }
                }
            }
            Pown(x, _) => {
                if let Constant(_) = &x.kind {
                    let val = expr.evaluate();
                    if val.len() <= 1 {
                        *expr = Expr::new(Constant(box val));
                    }
                }
            }
            _ => (),
        }
    }
}

// Does the following tasks:
// - Assign ids to exprs.
// - Assign ids to rel leaves so that they match with the indices in `EvalResult`.
pub struct AssignIdStage1<'a> {
    next_expr_id: ExprId,
    next_site: u8,
    site_map: SiteMap,
    visited_exprs: HashSet<&'a Expr>,
    next_rel_id: RelId,
    visited_rels: HashSet<&'a Rel>,
}

impl<'a> AssignIdStage1<'a> {
    pub fn new() -> Self {
        AssignIdStage1 {
            next_expr_id: 2, // 0 for x, 1 for y
            next_site: 0,
            site_map: HashMap::new(),
            visited_exprs: HashSet::new(),
            next_rel_id: 0,
            visited_rels: HashSet::new(),
        }
    }

    fn expr_can_perform_cut(kind: &ExprKind) -> bool {
        use {BinaryOp::*, ExprKind::*, UnaryOp::*};
        matches!(kind,
            Unary(Ceil, _)
            | Unary(Floor, _)
            | Unary(Recip, _)
            | Unary(Sign, _)
            | Unary(Tan, _)
            | Binary(Atan2, _, _)
            | Binary(Div, _, _)
            | Binary(Mod, _, _)
            | Pown(_, _))
    }
}

impl<'a> Visit<'a> for AssignIdStage1<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        traverse_expr(self, expr);

        match self.visited_exprs.get(expr) {
            Some(visited) => {
                let id = visited.id.get();
                expr.id.set(id);

                if let Some(site) = self.site_map.get_mut(&id) {
                    if site.is_none() && self.next_site <= 31 {
                        *site = Some(self.next_site as u8);
                        self.next_site += 1;
                    }
                }
            }
            _ => {
                let id = match &expr.kind {
                    ExprKind::X => 0,
                    ExprKind::Y => 1,
                    _ => {
                        let id = self.next_expr_id;
                        self.next_expr_id += 1;
                        id
                    }
                };
                expr.id.set(id);

                if Self::expr_can_perform_cut(&expr.kind) {
                    self.site_map.insert(id, None);
                }

                self.visited_exprs.insert(expr);
            }
        }
    }

    fn visit_rel(&mut self, rel: &'a Rel) {
        traverse_rel(self, rel);

        if let RelKind::Equality(_, _, _) = rel.kind {
            match self.visited_rels.get(rel) {
                Some(visited) => {
                    let id = visited.id.get();
                    rel.id.set(id);
                }
                _ => {
                    rel.id.set(self.next_rel_id);
                    self.next_rel_id += 1;
                    self.visited_rels.insert(rel);
                }
            }
        }
    }
}

// Does the following tasks:
// - Assign sites to rels.
// - Assign ids to rel nodes.
pub struct AssignIdStage2<'a> {
    next_expr_id: ExprId,
    site_map: SiteMap,
    next_rel_id: RelId,
    visited_rels: HashSet<&'a Rel>,
}

impl<'a> AssignIdStage2<'a> {
    pub fn new(stage1: AssignIdStage1<'a>) -> Self {
        AssignIdStage2 {
            next_expr_id: stage1.next_expr_id,
            site_map: stage1.site_map,
            next_rel_id: stage1.next_rel_id,
            visited_rels: stage1.visited_rels,
        }
    }
}

impl<'a> Visit<'a> for AssignIdStage2<'a> {
    fn visit_expr(&mut self, expr: &'a Expr) {
        traverse_expr(self, expr);

        if let Some(site) = self.site_map.get(&expr.id.get()) {
            expr.site.set(*site);
        }
    }

    fn visit_rel(&mut self, rel: &'a Rel) {
        traverse_rel(self, rel);

        match self.visited_rels.get(rel) {
            Some(visited) => {
                let id = visited.id.get();
                rel.id.set(id);
            }
            _ => {
                rel.id.set(self.next_rel_id);
                self.next_rel_id += 1;
                self.visited_rels.insert(rel);
            }
        }
    }
}

// Collects `StaticExpr`s (except the ones for X and Y) and `StaticRel`s,
// sorted topologically.
pub struct CollectStatic {
    exprs: Vec<Option<StaticExpr>>,
    rels: Vec<Option<StaticRel>>,
}

impl CollectStatic {
    pub fn new(stage2: AssignIdStage2) -> Self {
        Self {
            exprs: vec![None; (stage2.next_expr_id - 2) as usize],
            rels: vec![None; stage2.next_rel_id as usize],
        }
    }

    pub fn exprs_rels(self) -> (Vec<StaticExpr>, Vec<StaticRel>) {
        (
            self.exprs.into_iter().collect::<Option<Vec<_>>>().unwrap(),
            self.rels.into_iter().collect::<Option<Vec<_>>>().unwrap(),
        )
    }
}

impl<'a> Visit<'a> for CollectStatic {
    fn visit_expr(&mut self, expr: &'a Expr) {
        use ExprKind::*;
        traverse_expr(self, expr);

        if expr.id.get() < 2 {
            return;
        }

        let i = (expr.id.get() - 2) as usize;
        if self.exprs[i].is_none() {
            self.exprs[i] = Some(StaticExpr {
                site: expr.site.get(),
                kind: match &expr.kind {
                    Constant(x) => StaticExprKind::Constant(x.clone()),
                    Unary(op, x) => StaticExprKind::Unary(*op, x.id.get()),
                    Binary(op, x, y) => StaticExprKind::Binary(*op, x.id.get(), y.id.get()),
                    Pown(x, y) => StaticExprKind::Pown(x.id.get(), *y),
                    X | Y | Uninit => panic!(),
                },
            });
        }
    }

    fn visit_rel(&mut self, rel: &'a Rel) {
        use RelKind::*;
        traverse_rel(self, rel);

        let i = rel.id.get() as usize;
        if self.rels[i].is_none() {
            self.rels[i] = Some(StaticRel {
                kind: match &rel.kind {
                    Equality(op, x, y) => StaticRelKind::Equality(*op, x.id.get(), y.id.get()),
                    And(x, y) => StaticRelKind::And(x.id.get(), y.id.get()),
                    Or(x, y) => StaticRelKind::Or(x.id.get(), y.id.get()),
                },
            });
        }
    }
}
