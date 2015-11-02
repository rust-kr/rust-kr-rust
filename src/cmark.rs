use pulldown_cmark::Parser;
use pulldown_cmark::html::push_html;

pub fn to_html(s: &str) -> String {
    let parser = Parser::new(s);
    let mut buffer = String::new();
    push_html(&mut buffer, parser);
    buffer
}
