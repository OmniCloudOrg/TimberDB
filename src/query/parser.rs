// TimberDB: A high-performance distributed log database
// query/parser.rs - Query language parser

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until, take_while, take_while1},
    character::complete::{char, digit1, multispace0, multispace1},
    combinator::{map, map_res, opt, recognize, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
};
use thiserror::Error;

use crate::storage::block::LogEntry;

// Query parser errors
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Syntax error: {0}")]
    Syntax(String),
    
    #[error("Invalid field: {0}")]
    InvalidField(String),
    
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    
    #[error("Invalid operator: {0}")]
    InvalidOperator(String),
    
    #[error("Invalid date format: {0}")]
    InvalidDate(String),
}

// Comparison operators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Like,
    NotLike,
    In,
    NotIn,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
}

// Logical operators
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalOperator {
    And,
    Or,
}

// Query expression
#[derive(Debug, Clone)]
pub enum Expression {
    Comparison {
        field: String,
        operator: ComparisonOperator,
        value: Value,
    },
    Logical {
        operator: LogicalOperator,
        expressions: Vec<Expression>,
    },
    Grouped(Box<Expression>),
    None,
}

// Query value types
#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    DateTime(DateTime<Utc>),
    Duration(Duration),
    Array(Vec<Value>),
    Null,
}

// Query order direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    Asc,
    Desc,
}

// Query order by clause
#[derive(Debug, Clone)]
pub struct OrderBy {
    pub field: String,
    pub direction: OrderDirection,
}

// Parsed query
#[derive(Debug, Clone)]
pub struct Query {
    pub partitions: Vec<String>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub filter: Expression,
    pub limit: Option<usize>,
    pub order_by: Vec<OrderBy>,
    pub fields: Option<Vec<String>>,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            partitions: Vec::new(),
            time_range: None,
            filter: Expression::None,
            limit: None,
            order_by: Vec::new(),
            fields: None,
        }
    }
}

// Parse a query string into a Query object
pub fn parse_query(input: &str) -> Result<Query, ParseError> {
    match query_parser(input) {
        Ok((_, query)) => Ok(query),
        Err(e) => Err(ParseError::Syntax(format!("{:?}", e))),
    }
}

// Main query parser
fn query_parser(input: &str) -> IResult<&str, Query> {
    let (input, _) = multispace0(input)?;
    let (input, _) = tag_no_case("select")(input)?;
    let (input, _) = multispace1(input)?;
    
    // Parse SELECT fields
    let (input, fields) = alt((
        map(tag("*"), |_| None),
        map(field_list, Some),
    ))(input)?;
    
    // Parse FROM clause
    let (input, _) = multispace1(input)?;
    let (input, _) = tag_no_case("from")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, partitions) = partition_list(input)?;
    
    // Parse WHERE clause (optional)
    let (input, filter) = opt(preceded(
        pair(multispace1, tag_no_case("where")),
        preceded(multispace1, expression),
    ))(input)?;
    
    // Parse TIME RANGE clause (optional)
    let (input, time_range) = opt(preceded(
        pair(multispace1, tag_no_case("time")),
        preceded(
            multispace1,
            time_range_parser,
        ),
    ))(input)?;
    
    // Parse ORDER BY clause (optional)
    let (input, order_by) = opt(preceded(
        pair(multispace1, tag_no_case("order")),
        preceded(
            pair(multispace1, tag_no_case("by")),
            preceded(multispace1, order_by_list),
        ),
    ))(input)?;
    
    // Parse LIMIT clause (optional)
    let (input, limit) = opt(preceded(
        pair(multispace1, tag_no_case("limit")),
        preceded(multispace1, map_res(digit1, |s: &str| s.parse::<usize>())),
    ))(input)?;
    
    // Create the query object
    let query = Query {
        partitions,
        time_range,
        filter: filter.unwrap_or(Expression::None),
        limit,
        order_by: order_by.unwrap_or_else(Vec::new),
        fields,
    };
    
    Ok((input, query))
}

// Parse a list of fields
fn field_list(input: &str) -> IResult<&str, Vec<String>> {
    separated_list1(
        preceded(multispace0, char(',')),
        preceded(multispace0, identifier),
    )(input)
}

// Parse a list of partitions
fn partition_list(input: &str) -> IResult<&str, Vec<String>> {
    separated_list1(
        preceded(multispace0, char(',')),
        preceded(multispace0, identifier),
    )(input)
}

// Parse a time range
fn time_range_parser(input: &str) -> IResult<&str, (DateTime<Utc>, DateTime<Utc>)> {
    let (input, _) = tag_no_case("range")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;
    
    // Parse start time
    let (input, start_time) = datetime_value(input)?;
    
    let (input, _) = multispace0(input)?;
    let (input, _) = char(',')(input)?;
    let (input, _) = multispace0(input)?;
    
    // Parse end time
    let (input, end_time) = datetime_value(input)?;
    
    let (input, _) = multispace0(input)?;
    let (input, _) = char(')')(input)?;
    
    match (start_time, end_time) {
        (Value::DateTime(start), Value::DateTime(end)) => Ok((input, (start, end))),
        _ => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

// Parse an order by list
fn order_by_list(input: &str) -> IResult<&str, Vec<OrderBy>> {
    separated_list1(
        preceded(multispace0, char(',')),
        preceded(multispace0, order_by_item),
    )(input)
}

// Parse a single order by item
fn order_by_item(input: &str) -> IResult<&str, OrderBy> {
    let (input, field) = identifier(input)?;
    let (input, direction) = opt(preceded(
        multispace1,
        alt((
            value(OrderDirection::Asc, tag_no_case("asc")),
            value(OrderDirection::Desc, tag_no_case("desc")),
        )),
    ))(input)?;
    
    Ok((
        input,
        OrderBy {
            field,
            direction: direction.unwrap_or(OrderDirection::Asc),
        },
    ))
}

// Parse an expression
fn expression(input: &str) -> IResult<&str, Expression> {
    let (input, first_expr) = logical_term(input)?;
    
    let (input, rest) = many0(pair(
        preceded(multispace0, tag_no_case("or")),
        preceded(multispace0, logical_term),
    ))(input)?;
    
    if rest.is_empty() {
        Ok((input, first_expr))
    } else {
        let mut expressions = vec![first_expr];
        expressions.extend(rest.into_iter().map(|(_, expr)| expr));
        
        Ok((
            input,
            Expression::Logical {
                operator: LogicalOperator::Or,
                expressions,
            },
        ))
    }
}

// Parse a logical term (expressions connected by AND)
fn logical_term(input: &str) -> IResult<&str, Expression> {
    let (input, first_expr) = logical_factor(input)?;
    
    let (input, rest) = many0(pair(
        preceded(multispace0, tag_no_case("and")),
        preceded(multispace0, logical_factor),
    ))(input)?;
    
    if rest.is_empty() {
        Ok((input, first_expr))
    } else {
        let mut expressions = vec![first_expr];
        expressions.extend(rest.into_iter().map(|(_, expr)| expr));
        
        Ok((
            input,
            Expression::Logical {
                operator: LogicalOperator::And,
                expressions,
            },
        ))
    }
}

// Parse a logical factor (comparison or grouped expression)
fn logical_factor(input: &str) -> IResult<&str, Expression> {
    alt((
        delimited(
            pair(char('('), multispace0),
            expression,
            pair(multispace0, char(')')),
        ),
        comparison,
    ))(input)
}

// Parse a comparison expression
fn comparison(input: &str) -> IResult<&str, Expression> {
    let (input, field) = identifier(input)?;
    let (input, _) = multispace0(input)?;
    let (input, operator) = comparison_operator(input)?;
    let (input, _) = multispace0(input)?;
    let (input, value) = value_parser(input)?;
    
    Ok((
        input,
        Expression::Comparison {
            field,
            operator,
            value,
        },
    ))
}

// Parse a comparison operator
fn comparison_operator(input: &str) -> IResult<&str, ComparisonOperator> {
    alt((
        value(ComparisonOperator::Equal, tag("=")),
        value(ComparisonOperator::NotEqual, alt((tag("!="), tag("<>")))),
        value(ComparisonOperator::GreaterThanOrEqual, tag(">=")),
        value(ComparisonOperator::GreaterThan, tag(">")),
        value(ComparisonOperator::LessThanOrEqual, tag("<=")),
        value(ComparisonOperator::LessThan, tag("<")),
        value(ComparisonOperator::Like, tag_no_case("like")),
        value(ComparisonOperator::NotLike, tag_no_case("not like")),
        value(ComparisonOperator::In, tag_no_case("in")),
        value(ComparisonOperator::NotIn, tag_no_case("not in")),
        value(ComparisonOperator::Contains, tag_no_case("contains")),
        value(ComparisonOperator::NotContains, tag_no_case("not contains")),
        value(ComparisonOperator::StartsWith, tag_no_case("starts with")),
        value(ComparisonOperator::EndsWith, tag_no_case("ends with")),
    ))(input)
}

// Parse a value
fn value_parser(input: &str) -> IResult<&str, Value> {
    alt((
        array_value,
        string_value,
        boolean_value,
        null_value,
        datetime_value,
        duration_value,
        number_value,
    ))(input)
}

// Parse a string value
fn string_value(input: &str) -> IResult<&str, Value> {
    alt((
        // Double-quoted string
        map(
            delimited(
                char('"'),
                take_until("\""),
                char('"'),
            ),
            |s: &str| Value::String(s.to_string()),
        ),
        // Single-quoted string
        map(
            delimited(
                char('\''),
                take_until("'"),
                char('\''),
            ),
            |s: &str| Value::String(s.to_string()),
        ),
    ))(input)
}

// Parse a number value
fn number_value(input: &str) -> IResult<&str, Value> {
    map_res(
        recognize(
            pair(
                opt(char('-')),
                alt((
                    // Decimal with optional decimal part
                    recognize(
                        pair(
                            digit1,
                            opt(preceded(char('.'), digit1)),
                        ),
                    ),
                    // Just decimal part
                    recognize(
                        preceded(char('.'), digit1),
                    ),
                )),
            ),
        ),
        |s: &str| s.parse::<f64>().map(Value::Number),
    )(input)
}

// Parse a boolean value
fn boolean_value(input: &str) -> IResult<&str, Value> {
    alt((
        value(Value::Boolean(true), tag_no_case("true")),
        value(Value::Boolean(false), tag_no_case("false")),
    ))(input)
}

// Parse a null value
fn null_value(input: &str) -> IResult<&str, Value> {
    value(Value::Null, tag_no_case("null"))(input)
}

// Parse a datetime value
fn datetime_value(input: &str) -> IResult<&str, Value> {
    // ISO 8601 format
    let (input, datetime_str) = recognize(
        tuple((
            digit1,
            char('-'),
            digit1,
            char('-'),
            digit1,
            opt(tuple((
                char('T'),
                digit1,
                char(':'),
                digit1,
                opt(tuple((
                    char(':'),
                    digit1,
                    opt(tuple((
                        char('.'),
                        digit1,
                    ))),
                ))),
                opt(alt((
                    char('Z'),
                    tuple((
                        alt((char('+'), char('-'))),
                        digit1,
                        opt(tuple((
                            char(':'),
                            digit1,
                        ))),
                    )),
                ))),
            ))),
        ))
    )(input)?;
    
    // Parse the datetime string
    match chrono::DateTime::parse_from_rfc3339(datetime_str) {
        Ok(dt) => Ok((input, Value::DateTime(dt.with_timezone(&Utc)))),
        Err(_) => Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        ))),
    }
}

// Parse a duration value
fn duration_value(input: &str) -> IResult<&str, Value> {
    let (input, duration_str) = delimited(
        tag("duration("),
        take_until(")"),
        char(')'),
    )(input)?;
    
    // Simple duration parser that handles "1h", "30m", "10s", etc.
    let mut total_seconds = 0u64;
    let mut current_value = 0u64;
    let mut chars = duration_str.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c.is_digit(10) {
            current_value = current_value * 10 + c.to_digit(10).unwrap() as u64;
        } else {
            match c {
                's' => total_seconds += current_value,
                'm' => total_seconds += current_value * 60,
                'h' => total_seconds += current_value * 3600,
                'd' => total_seconds += current_value * 86400,
                'w' => total_seconds += current_value * 604800,
                _ => {
                    return Err(nom::Err::Error(nom::error::Error::new(
                        input,
                        nom::error::ErrorKind::Tag,
                    )))
                }
            }
            
            current_value = 0;
        }
    }
    
    Ok((input, Value::Duration(Duration::from_secs(total_seconds))))
}

// Parse an array value
fn array_value(input: &str) -> IResult<&str, Value> {
    map(
        delimited(
            pair(char('['), multispace0),
            separated_list0(
                delimited(multispace0, char(','), multispace0),
                value_parser,
            ),
            pair(multispace0, char(']')),
        ),
        Value::Array,
    )(input)
}

// Parse an identifier
fn identifier(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|c| {
            is_alphanumeric(c) || c == '_' || c == '.'
        }),
        |s: &str| s.to_string(),
    )(input)
}

// Helper function to check if a character is alphanumeric
fn is_alphanumeric(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

// Evaluate an expression against a log entry
pub fn evaluate_expression(expr: &Expression, entry: &LogEntry) -> bool {
    match expr {
        Expression::Comparison {
            field,
            operator,
            value,
        } => {
            let entry_value = get_field_value(field, entry);
            compare_values(operator, &entry_value, value)
        }
        Expression::Logical {
            operator,
            expressions,
        } => {
            match operator {
                LogicalOperator::And => expressions
                    .iter()
                    .all(|expr| evaluate_expression(expr, entry)),
                LogicalOperator::Or => expressions
                    .iter()
                    .any(|expr| evaluate_expression(expr, entry)),
            }
        }
        Expression::Grouped(expr) => evaluate_expression(expr, entry),
        Expression::None => true,
    }
}

// Get a field value from a log entry
fn get_field_value(field: &str, entry: &LogEntry) -> Value {
    match field {
        "timestamp" => Value::DateTime(entry.timestamp),
        "source" => Value::String(entry.source.clone()),
        "message" => Value::String(entry.message.clone()),
        _ => {
            // Check if it's a tag
            if field.starts_with("tags.") {
                let tag_name = &field[5..];
                if let Some(tag_value) = entry.tags.get(tag_name) {
                    Value::String(tag_value.clone())
                } else {
                    Value::Null
                }
            } else {
                Value::Null
            }
        }
    }
}

// Compare two values using the given operator
fn compare_values(op: &ComparisonOperator, left: &Value, right: &Value) -> bool {
    match op {
        ComparisonOperator::Equal => values_equal(left, right),
        ComparisonOperator::NotEqual => !values_equal(left, right),
        ComparisonOperator::GreaterThan => values_greater_than(left, right),
        ComparisonOperator::GreaterThanOrEqual => {
            values_greater_than(left, right) || values_equal(left, right)
        }
        ComparisonOperator::LessThan => values_less_than(left, right),
        ComparisonOperator::LessThanOrEqual => {
            values_less_than(left, right) || values_equal(left, right)
        }
        ComparisonOperator::Like => values_like(left, right),
        ComparisonOperator::NotLike => !values_like(left, right),
        ComparisonOperator::In => values_in(left, right),
        ComparisonOperator::NotIn => !values_in(left, right),
        ComparisonOperator::Contains => values_contains(left, right),
        ComparisonOperator::NotContains => !values_contains(left, right),
        ComparisonOperator::StartsWith => values_starts_with(left, right),
        ComparisonOperator::EndsWith => values_ends_with(left, right),
    }
}

// Check if two values are equal
fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(l), Value::String(r)) => l == r,
        (Value::Number(l), Value::Number(r)) => (l - r).abs() < f64::EPSILON,
        (Value::Boolean(l), Value::Boolean(r)) => l == r,
        (Value::DateTime(l), Value::DateTime(r)) => l == r,
        (Value::Duration(l), Value::Duration(r)) => l == r,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}

// Check if left value is greater than right value
fn values_greater_than(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(l), Value::Number(r)) => l > r,
        (Value::DateTime(l), Value::DateTime(r)) => l > r,
        (Value::Duration(l), Value::Duration(r)) => l > r,
        _ => false,
    }
}

// Check if left value is less than right value
fn values_less_than(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(l), Value::Number(r)) => l < r,
        (Value::DateTime(l), Value::DateTime(r)) => l < r,
        (Value::Duration(l), Value::Duration(r)) => l < r,
        _ => false,
    }
}

// Check if left value is like right value (simple pattern matching)
fn values_like(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(l), Value::String(r)) => {
            let pattern = r.replace('%', ".*").replace('_', ".");
            let regex = regex::Regex::new(&format!("^{}$", pattern)).unwrap();
            regex.is_match(l)
        }
        _ => false,
    }
}

// Check if left value is in right value (array)
fn values_in(left: &Value, right: &Value) -> bool {
    match right {
        Value::Array(values) => values.iter().any(|v| values_equal(left, v)),
        _ => false,
    }
}

// Check if left value contains right value
fn values_contains(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(l), Value::String(r)) => l.contains(r),
        _ => false,
    }
}

// Check if left value starts with right value
fn values_starts_with(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(l), Value::String(r)) => l.starts_with(r),
        _ => false,
    }
}

// Check if left value ends with right value
fn values_ends_with(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(l), Value::String(r)) => l.ends_with(r),
        _ => false,
    }
}