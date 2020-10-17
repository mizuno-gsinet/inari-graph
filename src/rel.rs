use crate::interval_set::*;

pub type ExprId = u32;
pub const UNINIT_EXPR_ID: ExprId = ExprId::MAX;

pub type RelId = u32;
pub const UNINIT_REL_ID: RelId = RelId::MAX;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnaryOp {
    Abs,
    Acos,
    Acosh,
    Asin,
    Asinh,
    Atan,
    Atanh,
    Ceil,
    Cos,
    Cosh,
    Exp,
    Exp10,
    Exp2,
    Floor,
    Ln,
    Log10,
    Log2,
    Neg,
    Recip,
    Sign,
    Sin,
    SinOverX,
    Sinh,
    Sqr,
    Sqrt,
    Tan,
    Tanh,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BinaryOp {
    Add,
    Atan2,
    Div,
    Max,
    Min,
    Mod,
    Mul,
    Pow,
    Sub,
}

#[derive(Clone, Debug)]
pub enum StaticExprKind {
    Constant(Box<TupperIntervalSet>),
    X,
    Y,
    Unary(UnaryOp, ExprId),
    Binary(BinaryOp, ExprId, ExprId),
    Pown(ExprId, i32),
}

#[derive(Clone, Debug)]
pub struct StaticExpr {
    pub site: Option<u8>,
    pub kind: StaticExprKind,
}

impl StaticExpr {
    pub fn evaluate(&self, ts: &[TupperIntervalSet]) -> TupperIntervalSet {
        use {BinaryOp::*, StaticExprKind::*, UnaryOp::*};
        match &self.kind {
            Constant(x) => *x.clone(),
            Unary(Abs, x) => ts[*x as usize].abs(),
            Unary(Acos, x) => ts[*x as usize].acos(),
            Unary(Acosh, x) => ts[*x as usize].acosh(),
            Unary(Asin, x) => ts[*x as usize].asin(),
            Unary(Asinh, x) => ts[*x as usize].asinh(),
            Unary(Atan, x) => ts[*x as usize].atan(),
            Unary(Atanh, x) => ts[*x as usize].atanh(),
            Unary(Ceil, x) => ts[*x as usize].ceil(self.site),
            Unary(Cos, x) => ts[*x as usize].cos(),
            Unary(Cosh, x) => ts[*x as usize].cosh(),
            Unary(Exp, x) => ts[*x as usize].exp(),
            Unary(Exp10, x) => ts[*x as usize].exp10(),
            Unary(Exp2, x) => ts[*x as usize].exp2(),
            Unary(Floor, x) => ts[*x as usize].floor(self.site),
            Unary(Ln, x) => ts[*x as usize].ln(),
            Unary(Log10, x) => ts[*x as usize].log10(),
            Unary(Log2, x) => ts[*x as usize].log2(),
            Unary(Neg, x) => -&ts[*x as usize],
            Unary(Recip, x) => ts[*x as usize].recip(self.site),
            Unary(Sign, x) => ts[*x as usize].sign(self.site),
            Unary(Sin, x) => ts[*x as usize].sin(),
            Unary(SinOverX, x) => ts[*x as usize].sin_over_x(),
            Unary(Sinh, x) => ts[*x as usize].sinh(),
            Unary(Sqr, x) => ts[*x as usize].sqr(),
            Unary(Sqrt, x) => ts[*x as usize].sqrt(),
            Unary(Tan, x) => ts[*x as usize].tan(self.site),
            Unary(Tanh, x) => ts[*x as usize].tanh(),
            Binary(Add, x, y) => &ts[*x as usize] + &ts[*y as usize],
            Binary(Atan2, x, y) => ts[*x as usize].atan2(&ts[*y as usize], self.site),
            Binary(Div, x, y) => ts[*x as usize].div(&ts[*y as usize], self.site),
            Binary(Max, x, y) => ts[*x as usize].max(&ts[*y as usize]),
            Binary(Min, x, y) => ts[*x as usize].min(&ts[*y as usize]),
            Binary(Mod, x, y) => ts[*x as usize].rem_euclid(&ts[*y as usize], self.site),
            Binary(Mul, x, y) => &ts[*x as usize] * &ts[*y as usize],
            Binary(Pow, x, y) => ts[*x as usize].pow(&ts[*y as usize], self.site),
            Binary(Sub, x, y) => &ts[*x as usize] - &ts[*y as usize],
            Pown(x, y) => ts[*x as usize].pown(*y, self.site),
            X | Y => panic!("this expression cannot be evaluated"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RelOp {
    Eq,
    Ge,
    Gt,
    Le,
    Lt,
}

#[derive(Clone, Debug)]
pub enum StaticRelKind {
    Atomic(RelOp, ExprId, ExprId),
    And(RelId, RelId),
    Or(RelId, RelId),
}

#[derive(Clone, Debug)]
pub struct StaticRel {
    pub kind: StaticRelKind,
}

impl StaticRel {
    pub fn evaluate(&self, ts: &[TupperIntervalSet]) -> DecSignSet {
        use {RelOp::*, StaticRelKind::*};
        match &self.kind {
            Atomic(Eq, x, y) => ts[*x as usize].eq(&ts[*y as usize]),
            Atomic(Ge, x, y) => ts[*x as usize].ge(&ts[*y as usize]),
            Atomic(Gt, x, y) => ts[*x as usize].gt(&ts[*y as usize]),
            Atomic(Le, x, y) => ts[*x as usize].le(&ts[*y as usize]),
            Atomic(Lt, x, y) => ts[*x as usize].lt(&ts[*y as usize]),
            And(_, _) | Or(_, _) => panic!("compound relation cannot be evaluated"),
        }
    }
}
