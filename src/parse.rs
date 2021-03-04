use crate::{
    ast::{Form, FormKind, RelOp, Term, TermKind},
    context::{Context, InputWithContext},
    interval_set::TupperIntervalSet,
};
use inari::dec_interval;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit0, digit1, satisfy, space0},
    combinator::{all_consuming, map, map_opt, opt, recognize, value},
    error::VerboseError,
    multi::{fold_many0, fold_many1, many0_count},
    sequence::{delimited, pair, preceded, separated_pair, terminated},
    Err as NomErr, IResult,
};

type ParseResult<'a, O> = IResult<InputWithContext<'a>, O, VerboseError<InputWithContext<'a>>>;

fn identifier_head(i: InputWithContext) -> ParseResult<char> {
    satisfy(|c| c.is_alphabetic())(i)
}

fn identifier_tail(i: InputWithContext) -> ParseResult<&str> {
    map(
        recognize(many0_count(satisfy(|c| {
            c.is_alphanumeric() || c == '_' || c == '\''
        }))),
        |s: InputWithContext| s.i,
    )(i)
}

fn identifier(i: InputWithContext) -> ParseResult<&str> {
    map(recognize(pair(identifier_head, identifier_tail)), |s| s.i)(i)
}

fn decimal_literal(i: InputWithContext) -> ParseResult<&str> {
    map(
        alt((
            // "12", "12." or "12.3"
            recognize(pair(digit1, opt(pair(char('.'), digit0)))),
            // ".3"
            recognize(pair(char('.'), digit1)),
        )),
        |s: InputWithContext| s.i,
    )(i)
}

fn decimal_constant(i: InputWithContext) -> ParseResult<Term> {
    map(decimal_literal, |s| {
        let s = ["[", s, ",", s, "]"].concat();
        let x = TupperIntervalSet::from(dec_interval!(&s).unwrap());
        Term::new(TermKind::Constant(Box::new(x)))
    })(i)
}

fn named_constant(i: InputWithContext) -> ParseResult<Term> {
    let ctx = i.ctx;
    map_opt(identifier, move |s| ctx.get_constant(s))(i)
}

/// Nonempty, comma-separated list of terms.
fn term_list(i: InputWithContext) -> ParseResult<Vec<Term>> {
    let (i, x) = term(i)?;

    fold_many0(
        preceded(delimited(space0, char(','), space0), term),
        vec![x],
        |mut xs, x| {
            xs.push(x);
            xs
        },
    )(i)
}

fn function_application(i: InputWithContext) -> ParseResult<Term> {
    let ctx = i.ctx;

    map_opt(
        pair(
            identifier,
            delimited(
                delimited(space0, char('('), space0),
                term_list,
                preceded(space0, char(')')),
            ),
        ),
        move |(s, args)| ctx.apply(s, args),
    )(i)
}

fn variable(i: InputWithContext) -> ParseResult<Term> {
    map(identifier, |s| Term::new(TermKind::Var(s.into())))(i)
}

fn primary_term(i: InputWithContext) -> ParseResult<Term> {
    let ctx = i.ctx;

    alt((
        decimal_constant,
        named_constant,
        function_application,
        variable,
        delimited(
            terminated(char('('), space0),
            term,
            preceded(space0, char(')')),
        ),
        map(
            delimited(
                terminated(char('['), space0),
                term_list,
                preceded(space0, char(']')),
            ),
            |xs| Term::new(TermKind::List(xs)),
        ),
        map_opt(
            delimited(
                terminated(char('|'), space0),
                term,
                preceded(space0, char('|')),
            ),
            move |x| ctx.apply("abs", vec![x]),
        ),
        map_opt(
            delimited(
                terminated(char('⌈'), space0),
                term,
                preceded(space0, char('⌉')),
            ),
            move |x| ctx.apply("ceil", vec![x]),
        ),
        map_opt(
            delimited(
                terminated(char('⌊'), space0),
                term,
                preceded(space0, char('⌋')),
            ),
            move |x| ctx.apply("floor", vec![x]),
        ),
    ))(i)
}

// ^ is right-associative: x^y^z is the same as x^(y^z).
fn power_term(i: InputWithContext) -> ParseResult<Term> {
    let ctx = i.ctx;

    alt((
        map(
            separated_pair(
                primary_term,
                delimited(space0, char('^'), space0),
                unary_term,
            ),
            move |(x, y)| ctx.apply("^", vec![x, y]).unwrap(),
        ),
        primary_term,
    ))(i)
}

fn unary_term(i: InputWithContext) -> ParseResult<Term> {
    let ctx = i.ctx;

    alt((
        preceded(pair(char('+'), space0), unary_term),
        map(preceded(pair(char('-'), space0), unary_term), move |x| {
            ctx.apply("-", vec![x]).unwrap()
        }),
        power_term,
    ))(i)
}

fn multiplicative_term(i: InputWithContext) -> ParseResult<Term> {
    let ctx = i.ctx;
    let (i, x) = unary_term(i)?;

    fold_many0(
        alt((
            // x * y
            // x / y
            pair(
                delimited(
                    space0,
                    alt((value("*", char('*')), value("/", char('/')))),
                    space0,
                ),
                unary_term,
            ),
            // x y
            pair(value("*", space0), power_term),
        )),
        x,
        move |xs, (op, y)| ctx.apply(op, vec![xs, y]).unwrap(),
    )(i)
}

fn additive_term(i: InputWithContext) -> ParseResult<Term> {
    let ctx = i.ctx;
    let (i, x) = multiplicative_term(i)?;

    fold_many0(
        pair(
            delimited(
                space0,
                alt((value("+", char('+')), value("-", char('-')))),
                space0,
            ),
            multiplicative_term,
        ),
        x,
        move |xs, (op, y)| ctx.apply(op, vec![xs, y]).unwrap(),
    )(i)
}

fn term(i: InputWithContext) -> ParseResult<Term> {
    additive_term(i)
}

// (In)equalities can be chained: x < y < z is the same as x < y && y < z.
fn equality(i: InputWithContext) -> ParseResult<Form> {
    // `acc` is a pair of `Vec<RelOp>` and `Vec<Term>` that store
    // lists of equality operators and their operands, respectively.
    // `acc.1.len() == acc.0.len() + 1` holds.
    let (i, acc) = map(term, |x| (vec![], vec![x]))(i)?;

    map(
        fold_many1(
            pair(
                delimited(
                    space0,
                    alt((
                        value(RelOp::Eq, char('=')),
                        value(RelOp::Ge, alt((tag(">="), tag("≥")))),
                        value(RelOp::Gt, char('>')),
                        value(RelOp::Le, alt((tag("<="), tag("≤")))),
                        value(RelOp::Lt, char('<')),
                    )),
                    space0,
                ),
                term,
            ),
            acc,
            |mut acc, (op, y)| {
                acc.0.push(op);
                acc.1.push(y);
                acc
            },
        ),
        |acc| {
            let op = acc.0[0];
            let x = acc.1[0].clone();
            let y = acc.1[1].clone();
            let mut f = Form::new(FormKind::Atomic(op, Box::new(x), Box::new(y)));
            for i in 1..acc.0.len() {
                let op = acc.0[i];
                let x = acc.1[i].clone();
                let y = acc.1[i + 1].clone();
                let f2 = Form::new(FormKind::Atomic(op, Box::new(x), Box::new(y)));
                f = Form::new(FormKind::And(Box::new(f), Box::new(f2)));
            }
            f
        },
    )(i)
}

fn primary_form(i: InputWithContext) -> ParseResult<Form> {
    alt((
        delimited(
            terminated(char('('), space0),
            form,
            preceded(space0, char(')')),
        ),
        equality,
    ))(i)
}

// Inputs like "!y < x", are permitted too.
fn not_form(i: InputWithContext) -> ParseResult<Form> {
    alt((
        map(preceded(pair(char('!'), space0), not_form), |x| {
            Form::new(FormKind::Not(Box::new(x)))
        }),
        primary_form,
    ))(i)
}

fn and_form(i: InputWithContext) -> ParseResult<Form> {
    let (i, x) = not_form(i)?;

    fold_many0(
        preceded(delimited(space0, tag("&&"), space0), not_form),
        x,
        |xs, y| Form::new(FormKind::And(Box::new(xs), Box::new(y))),
    )(i)
}

fn or_form(i: InputWithContext) -> ParseResult<Form> {
    let (i, x) = and_form(i)?;

    fold_many0(
        preceded(delimited(space0, tag("||"), space0), and_form),
        x,
        |xs, y| Form::new(FormKind::Or(Box::new(xs), Box::new(y))),
    )(i)
}

fn form(i: InputWithContext) -> ParseResult<Form> {
    or_form(i)
}

/// Parses a formula.
pub fn parse(i: &str, ctx: &Context) -> Result<Form, String> {
    let i = InputWithContext::new(i, ctx);
    match all_consuming(delimited(space0, form, space0))(i.clone()) {
        Ok((InputWithContext { i: "", ctx: _ }, x)) => Ok(x),
        Err(NomErr::Error(e)) | Err(NomErr::Failure(e)) => Err(convert_error(i, e)),
        _ => unreachable!(),
    }
}

// Copied from `nom::error::convert_error`.
#[allow(clippy::naive_bytecount)]
fn convert_error(input: InputWithContext, e: VerboseError<InputWithContext>) -> String {
    use nom::Offset;

    let input = input.i;
    let substring = e.errors.first().unwrap().0.i;
    let offset = input.offset(substring);

    let prefix = &input.as_bytes()[..offset];

    // Count the number of newlines in the first `offset` bytes of input
    let line_number = prefix.iter().filter(|&&b| b == b'\n').count() + 1;

    // Find the line that includes the subslice:
    // Find the *last* newline before the substring starts
    let line_begin = prefix
        .iter()
        .rev()
        .position(|&b| b == b'\n')
        .map(|pos| offset - pos)
        .unwrap_or(0);

    // Find the full line after that newline
    let line = input[line_begin..]
        .lines()
        .next()
        .unwrap_or(&input[line_begin..])
        .trim_end();

    // The (1-indexed) column number is the offset of our substring into that line
    let column_number = line.offset(substring) + 1;

    format!(
        "at line {line_number}:\n\
               {line}\n\
               {caret:>column$}\n\n",
        line_number = line_number,
        line = line,
        caret = '^',
        column = column_number,
    )
}

#[cfg(test)]
mod tests {
    use crate::context::Context;

    #[test]
    fn parse_term() {
        test_parse_term("e", "@");
        test_parse_term("gamma", "@");
        test_parse_term("γ", "@");
        test_parse_term("pi", "@");
        test_parse_term("π", "@");
        test_parse_term("[x, y, z]", "(List x y z)");
        test_parse_term("|x|", "(Abs x)");
        test_parse_term("⌈x⌉", "(Ceil x)");
        test_parse_term("⌊x⌋", "(Floor x)");
        test_parse_term("abs(x)", "(Abs x)");
        test_parse_term("acos(x)", "(Acos x)");
        test_parse_term("acosh(x)", "(Acosh x)");
        test_parse_term("Ai(x)", "(AiryAi x)");
        test_parse_term("Ai'(x)", "(AiryAiPrime x)");
        test_parse_term("Bi(x)", "(AiryBi x)");
        test_parse_term("Bi'(x)", "(AiryBiPrime x)");
        test_parse_term("asin(x)", "(Asin x)");
        test_parse_term("asinh(x)", "(Asinh x)");
        test_parse_term("atan(x)", "(Atan x)");
        test_parse_term("atanh(x)", "(Atanh x)");
        test_parse_term("ceil(x)", "(Ceil x)");
        test_parse_term("Chi(x)", "(Chi x)");
        test_parse_term("Ci(x)", "(Ci x)");
        test_parse_term("cos(x)", "(Cos x)");
        test_parse_term("cosh(x)", "(Cosh x)");
        test_parse_term("psi(x)", "(Digamma x)");
        test_parse_term("ψ(x)", "(Digamma x)");
        test_parse_term("Ei(x)", "(Ei x)");
        test_parse_term("erf(x)", "(Erf x)");
        test_parse_term("erfc(x)", "(Erfc x)");
        test_parse_term("erfi(x)", "(Erfi x)");
        test_parse_term("exp(x)", "(Exp x)");
        test_parse_term("floor(x)", "(Floor x)");
        test_parse_term("C(x)", "(FresnelC x)");
        test_parse_term("S(x)", "(FresnelS x)");
        test_parse_term("Gamma(x)", "(Gamma x)");
        test_parse_term("Γ(x)", "(Gamma x)");
        test_parse_term("li(x)", "(Li x)");
        test_parse_term("ln(x)", "(Ln x)");
        test_parse_term("log(x)", "(Log10 x)");
        test_parse_term("Shi(x)", "(Shi x)");
        test_parse_term("Si(x)", "(Si x)");
        test_parse_term("sign(x)", "(Sign x)");
        test_parse_term("sin(x)", "(Sin x)");
        test_parse_term("sinh(x)", "(Sinh x)");
        test_parse_term("sqrt(x)", "(Sqrt x)");
        test_parse_term("tan(x)", "(Tan x)");
        test_parse_term("tanh(x)", "(Tanh x)");
        test_parse_term("atan2(y, x)", "(Atan2 y x)");
        test_parse_term("I(n, x)", "(BesselI n x)");
        test_parse_term("J(n, x)", "(BesselJ n x)");
        test_parse_term("K(n, x)", "(BesselK n x)");
        test_parse_term("Y(n, x)", "(BesselY n x)");
        test_parse_term("Gamma(a, x)", "(GammaInc a x)");
        test_parse_term("Γ(a, x)", "(GammaInc a x)");
        test_parse_term("log(b, x)", "(Log b x)");
        test_parse_term("mod(x, y)", "(Mod x y)");
        test_parse_term("gcd(x, y, z)", "(Gcd (Gcd x y) z)");
        test_parse_term("lcm(x, y, z)", "(Lcm (Lcm x y) z)");
        test_parse_term("max(x, y, z)", "(Max (Max x y) z)");
        test_parse_term("min(x, y, z)", "(Min (Min x y) z)");
        test_parse_term("ranked_max([x, y, z], k)", "(RankedMax (List x y z) k)");
        test_parse_term("ranked_min([x, y, z], k)", "(RankedMin (List x y z) k)");
        test_parse_term("x ^ y ^ z", "(Pow x (Pow y z))");
        test_parse_term("-x ^ -y", "(Neg (Pow x (Neg y)))");
        test_parse_term("+x", "x");
        test_parse_term("-x", "(Neg x)");
        test_parse_term("x y z", "(Mul (Mul x y) z)");
        test_parse_term("x * y * z", "(Mul (Mul x y) z)");
        test_parse_term("x / y / z", "(Div (Div x y) z)");
        test_parse_term("x + y + z", "(Add (Add x y) z)");
        test_parse_term("x - y - z", "(Sub (Sub x y) z)");
        test_parse_term("x + y z", "(Add x (Mul y z))");
        test_parse_term("(x + y) z", "(Mul (Add x y) z)");
    }

    fn test_parse_term(input: &str, expected: &str) {
        let f = super::parse(&format!("{} = 0", input), Context::builtin_context()).unwrap();
        assert_eq!(
            format!("(Eq {} @)", expected),
            format!("{}", f.dump_structure())
        );
    }

    #[test]
    fn parse_forms() {
        test_parse_form("x = y", "(Eq x y)");
        test_parse_form("x >= y", "(Ge x y)");
        test_parse_form("x ≥ y", "(Ge x y)");
        test_parse_form("x > y", "(Gt x y)");
        test_parse_form("x <= y", "(Le x y)");
        test_parse_form("x ≤ y", "(Le x y)");
        test_parse_form("x < y", "(Lt x y)");
        test_parse_form("x = y = z", "(And (Eq x y) (Eq y z))");
        test_parse_form("!x = y", "(Not (Eq x y))");
        test_parse_form("x = y && y = z", "(And (Eq x y) (Eq y z))");
        test_parse_form("x = y || y = z", "(Or (Eq x y) (Eq y z))");
        test_parse_form(
            "x = y || y = z && z = x",
            "(Or (Eq x y) (And (Eq y z) (Eq z x)))",
        );
        test_parse_form(
            "(x = y || y = z) && z = x",
            "(And (Or (Eq x y) (Eq y z)) (Eq z x))",
        );
    }

    fn test_parse_form(input: &str, expected: &str) {
        let f = super::parse(input, Context::builtin_context()).unwrap();
        assert_eq!(expected, format!("{}", f.dump_structure()));
    }
}
