// TODO block/endblock
#[deriving(Eq)]
pub enum Token {
    JinjaText(~str),
    JinjaVariable(~str),
}

#[deriving(Eq)]
pub struct Template {
    elements: ~[Token],
}

impl Template {
    pub fn new(s: &str) -> Template {
        fn find_then(s: &str, t: &str, f: &fn(j: uint)) -> bool {
            match s.find_str(t) {
                None => true,
                Some(j) => {
                    f(j);
                    false
                }
            }
        }

        let mut s = s;
        let mut elements = ~[];
        while s.len() > 0 {
            let i = s.find('{');
            let end = if i == None {
                true
            } else {
                let i = i.unwrap();
                if i + 1 >= s.len() {
                    true
                } else {
                    match s.char_at(i + 1) {
                        '{' => do find_then(s, "}}") |j| {
                            // TODO: filter
                            let b = s.slice_to(i);
                            elements.push(JinjaText(b.to_owned()));
                            let var = s.slice(i + 2, j);
                            let var = JinjaVariable(var.to_owned());
                            elements.push(var);
                            s = s.slice_from(j + 2);
                        },
                        _ => {
                            // TODO: "{% %}"
                            elements.push(JinjaText(~"{"));
                            s = s.slice_from(1);
                            continue
                        }
                    }
                }
            };
            if end {
                let elem = JinjaText(s.to_owned());
                elements.push(elem);
                break;
            }
        }
        Template {
            elements: elements,
        }
    }

    pub fn replace(&self, values: &[(&str, &str)]) -> ~str {
        let mut ret = ~[];
        for e in self.elements.iter() {
            match e {
                &JinjaText(ref t) => {
                    ret.push(t.as_slice());
                },
                &JinjaVariable(ref v) => {
                    for &(var, val) in values.iter() {
                        if v.equiv(&var) {
                            ret.push(val);
                            break;
                        }
                    }
                },
            }
        }
        ret.concat()
    }
}

#[cfg(test)]
mod tests {
    use super::Template;
    use super::{JinjaText, JinjaVariable};

    #[test]
    fn test_parse() {
        let text = "Hello {{name}}!";
        let output = Template::new(text);
        let expected = Template {
            elements: ~[
                JinjaText(~"Hello "),
                JinjaVariable(~"name"),
                JinjaText(~"!"),
            ],
        };
        assert_eq!(output, expected);
    }
}
