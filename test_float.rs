fn main() {
    let max_i64 = i64::MAX;
    let max_as_f64 = max_i64 as f64;
    let max_literal = 9223372036854775807.0f64;
    
    println\!("i64::MAX = {}", max_i64);
    println\!("i64::MAX as f64 = {}", max_as_f64);
    println\!("literal 9223372036854775807.0 = {}", max_literal);
    println\!("Are they equal? {}", max_as_f64 == max_literal);
    println\!("max_literal as i64 = {}", max_literal as i64);
    println\!("max_literal < 9223372036854775808.0? {}", max_literal < 9223372036854775808.0);
}
