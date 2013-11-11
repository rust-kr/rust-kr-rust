extern mod extra;
extern mod rustdoc;
extern mod http;

use std::rt::io::net::ip::{SocketAddr, Ipv4Addr};
use std::rt::io::{Writer, File};
use std::rt::io::fs::readdir;

use extra::time;

use http::server::{Config, Server, ServerUtil, Request, ResponseWriter};
use http::server::request::AbsolutePath;
use http::headers::content_type::MediaType;

mod jinja2;

// assumes utf-8
pub trait PercentDecoder {
    fn decode_percent(&self) -> ~str;
}

impl<'self> PercentDecoder for &'self str {
    fn decode_percent(&self) -> ~str {
        fn hex_to_u8(h: &Ascii) -> u8 {
            let h = h.to_byte();
            match h {
                0x30..0x39 => h - 0x30, // '0'..'9'
                0x41..0x46 => h - 0x41 + 10, // 'A'..'F'
                0x61..0x66 => h - 0x61 + 10, // 'a'..'f'
                _ => fail!("not a hex value")
            }
        }

        let mut buf: ~[u8] = ~[];
        let mut it = self.to_ascii().iter();
        loop {
            let c = it.next();
            match c {
                None => break,
                Some(c) => {
                    let c = c.to_byte();
                    if c == 0x25 {
                        let c1 = hex_to_u8(it.next().unwrap());
                        let c2 = hex_to_u8(it.next().unwrap());
                        let cc = c1 * 16 + c2;
                        buf.push(cc);
                    } else {
                        buf.push(c);
                    }
                }
            }
        }

        std::str::from_utf8_owned(buf)
    }
}

#[deriving(Clone)]
struct RustKrServer {
    doc_dir: ~str,
}

impl Server for RustKrServer {
    fn get_config(&self) -> Config {
        Config {
            bind_address: SocketAddr {
                ip: Ipv4Addr(127, 0, 0, 1),
                port: 8001,
            }
        }
    }

    fn handle_request(&self, r: &Request, w: &mut ResponseWriter) {
        let (title, content) = match r.request_uri {
            AbsolutePath(ref url) => {
                // remove '/'
                let title = url.slice_from(1);
                if self.is_bad_title(title) {
                    (~":p", ~"Bad title")
                } else {
                    let title = if title.len() == 0 {
                        ~"index"
                    } else {
                        title.decode_percent()
                    };
                    if title == ~"_pages" {
                        (~"모든 문서", self.list_pages())
                    } else {
                        let page = self.read_page(title);
                        (title, page)
                    }
                }
            },
            _ => {
                (~"???", ~"???")
            }
        };

        let template_path = Path::new("templates/default.html");
        let mut template_file = File::open(&template_path);
        let template = template_file.read_to_end();
        let template = std::str::from_utf8(template);
        let template = jinja2::Template::new(template);
        let values = [
            ("content", content.as_slice()),
            ("title", title.as_slice()),
        ];
        let output = template.replace(values);
        let output_b = output.as_bytes();

        w.headers.date = Some(time::now_utc());
        w.headers.content_length = Some(output_b.len());
        w.headers.content_type = Some(MediaType {
            type_: ~"text",
            subtype: ~"html",
            parameters: ~[(~"charset", ~"UTF-8")]
        });
        w.headers.server = Some(~"Example");

        w.write(output_b);
    }
}

impl RustKrServer {
    fn is_bad_title(&self, title: &str) -> bool {
        if title.contains_char('.') {
            return true;
        }
        if title.contains_char('/') {
            return true;
        }
        if !title.is_ascii() {
            return true;
        }

        false
    }

    fn read_page(&self, title: &str) -> ~str {
        let path = format!("{:s}/{:s}.md", self.doc_dir, title);
        let path = Path::new(path);
        if !path.exists() {
            return ~"No such page";
        }
        let mut f = File::open(&path);
        let text = f.read_to_end();
        let text = std::str::from_utf8(text);
        let md = rustdoc::html::markdown::Markdown(text);
        format!("{}", md)
    }

    pub fn list_pages(&self) -> ~str {
        let files = {
            let dir = Path::new(self.doc_dir.clone());
            if !dir.exists() {
                return ~"No pages found";
            }
            readdir(&dir)
        };
        let mut pages = ~[];
        for file in files.iter() {
            if file.is_dir() {
                continue;
            }
            match file.as_str() {
                None => continue,
                Some(s) => {
                    if s.ends_with(".md") {
                        pages.push(file.filestem_str().unwrap().to_owned());
                    }
                }
            }
        }
        if pages.len() > 0 {
            let mut ret = ~"<ul>\n";
            for page in pages.iter() {
                ret = ret + format!(r#"<li><a href="/{:s}">{:s}</a></li>"#, *page, *page);
            }
            ret = ret + "</ul>";
            ret
        } else {
            ~"No pages found"
        }
    }

    pub fn new(doc_dir: ~str) -> RustKrServer {
        RustKrServer {
            doc_dir: doc_dir,
        }
    }
}

fn main() {
    let server = RustKrServer::new(~"docs");
    server.serve_forever();
}

#[cfg(test)]
mod test {
    use super::PercentDecoder;

    fn compare(input: &str, output: &str) {
        assert_eq!(input.decode_percent(), output.to_owned());
    }
    #[test]
    fn decode_percent() {
        compare("abc", "abc");
        compare("a%20bc", "a bc");
        compare("a%2Fbc", "a/bc");
        compare("%EA%B0%80%EB%82%98%EB%8B%A4", "가나다");
    }
}
