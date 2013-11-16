extern mod extra;
extern mod http;
extern mod mustache;

use std::io::net::ip::{SocketAddr, Ipv4Addr, Port};
use std::io::{Writer, File};
use std::io::fs::readdir;

use extra::getopts;
use extra::time;

use http::server::{Config, Server, ServerUtil, Request, ResponseWriter};
use http::server::request::AbsolutePath;
use http::headers::content_type::MediaType;

mod markdown;

#[deriving(Encodable)]
struct Ctx {
    content: ~str,
    title: ~str,
}

// assumes utf-8
pub trait PercentDecoder {
    fn decode_percent(&self) -> Option<~str>;
}

impl<'self> PercentDecoder for &'self str {
    fn decode_percent(&self) -> Option<~str> {
        fn hex_to_u8(h: &Ascii) -> Option<u8> {
            let h = h.to_byte();
            let value = match h {
                0x30..0x39 => h - 0x30, // '0'..'9'
                0x41..0x46 => h - 0x41 + 10, // 'A'..'F'
                0x61..0x66 => h - 0x61 + 10, // 'a'..'f'
                _ => return None,
            };
            Some(value)
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
                        let c1 = do it.next().and_then |n| { hex_to_u8(n) };
                        let c1 = match c1 {
                            None => return None,
                            Some(c) => c,
                        };
                        let c2 = do it.next().and_then |n| { hex_to_u8(n) };
                        let c2 = match c2 {
                            None => return None,
                            Some(c) => c,
                        };
                        let cc = c1 * 16 + c2;
                        buf.push(cc);
                    } else {
                        buf.push(c);
                    }
                }
            }
        }

        std::str::from_utf8_owned_opt(buf)
    }
}

#[deriving(Clone)]
struct RustKrServer {
    doc_dir: ~str,
    port: Port,
}

impl Server for RustKrServer {
    fn get_config(&self) -> Config {
        Config {
            bind_address: SocketAddr {
                ip: Ipv4Addr(127, 0, 0, 1),
                port: self.port,
            }
        }
    }

    fn handle_request(&self, r: &Request, w: &mut ResponseWriter) {
        match r.request_uri {
            AbsolutePath(ref url) => {
                let url = url.as_slice();
                // remove '/'
                if url.char_at(0) != '/' {
                    // TODO is this possible?
                    self.show_bad_request(w);
                    return;
                }

                if url == "/" {
                    self.show_page(w, "index");
                    return;
                }

                let url = url.slice_from(1);
                match url.find('/') {
                    None => {
                        self.show_bad_request(w);
                    }
                    Some(i) => {
                        let prefix = url.slice_to(i);
                        let remaining = url.slice_from(i + 1);
                        match prefix {
                            "pages" => {
                                let title = remaining;
                                if self.is_bad_title(title) {
                                    self.show_bad_request(w);
                                    return;
                                }
                                let title = title.decode_percent();
                                match title {
                                    Some(title) => self.show_page(w, title),
                                    None => self.show_bad_request(w),
                                }
                            }
                            "static" => {
                                self.show_static_file(w, remaining);
                            }
                            _ => {
                                self.show_bad_request(w); // XXX
                            }
                        }
                    }
                }
            }
            _ => {
                // TODO
                self.show_bad_request(w);
            }
        }
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

    fn read_page(&self, title: &str) -> Option<~str> {
        let path = format!("{:s}/{:s}.md", self.doc_dir, title);
        let path = Path::new(path);
        if !path.exists() {
            return None;
        }
        let mut f = File::open(&path);
        let text = f.read_to_end();
        let text = std::str::from_utf8(text);
        let md = markdown::Markdown(text);
        Some(format!("{}", md))
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
                ret = ret + format!(r#"<li><a href="/pages/{:s}">{:s}</a></li>"#, *page, *page);
            }
            ret = ret + "</ul>";
            ret
        } else {
            ~"No pages found"
        }
    }

    fn show_bad_request(&self, w: &mut ResponseWriter) {
        let ctx = Ctx {
            title: ~"Bad request",
            content: ~"헐",
        };
        // TODO response code
        self.show_template(w, &ctx);
    }

    fn show_template(&self, w: &mut ResponseWriter, ctx: &Ctx) {
        let template_path = Path::new("templates/default.html");
        let mut template_file = File::open(&template_path);
        let template = template_file.read_to_end();
        let template = std::str::from_utf8(template);

        let output = mustache::render_str(template, ctx);
        let output_b = output.as_bytes();

        w.headers.date = Some(time::now_utc()); // TODO
        w.headers.content_length = Some(output_b.len());
        w.headers.content_type = Some(MediaType {
            type_: ~"text",
            subtype: ~"html",
            parameters: ~[(~"charset", ~"UTF-8")]
        });
        w.headers.server = Some(~"rust-kr-rust");

        w.write(output_b);
    }

    fn show_page(&self, w: &mut ResponseWriter, title: &str) {
        let (title, content) = match title {
            "_pages" => ("모든 문서", self.list_pages()),
            _ => {
                let content = self.read_page(title);
                match content {
                    Some(content) => (title, content),
                    None => (title, ~"No such page"),
                }
            }
        };
        let ctx = Ctx {
            title: title.to_owned(),
            content: content,
        };
        self.show_template(w, &ctx);
    }

    fn show_static_file(&self, w: &mut ResponseWriter, loc: &str) {
        let path = Path::new(format!("static/{}", loc));
        if !path.exists() {
            // TODO 404
            self.show_bad_request(w);
            return;
        }
        let mut f = File::open(&path);
        let f = f.read_to_end();

        let subtype = match path.extension_str() {
            Some("css") => ~"css",
            _ => ~"plain",
        };
        w.headers.date = Some(time::now_utc()); // TODO
        w.headers.content_length = Some(f.len());
        w.headers.content_type = Some(MediaType {
            type_: ~"text",
            subtype: subtype,
            parameters: ~[(~"charset", ~"UTF-8")]
        });
        w.headers.server = Some(~"rust-kr-rust");

        w.write(f);
    }

    pub fn new(doc_dir: ~str, port: Port) -> RustKrServer {
        RustKrServer {
            doc_dir: doc_dir,
            port: port,
        }
    }
}

fn main() {
    let opts = [
        getopts::optopt("p"),
    ];

    let args = std::os::args();
    let args = args.slice_from(1);
    let matches = getopts::getopts(args, opts).expect("Bad opts");

    let port = matches.opt_str("p").unwrap_or(~"8001");
    let port = from_str(port).expect("Port number");
    let server = RustKrServer::new(~"docs", port);
    server.serve_forever();
}

#[cfg(test)]
mod test {
    use super::PercentDecoder;

    fn compare(input: &str, output: &str) {
        assert_eq!(input.decode_percent().unwrap(), output.to_owned());
    }

    fn assert_none(input: &str) {
        assert_eq!(input.decode_percent(), None);
    }

    #[test]
    fn decode_percent() {
        compare("abc", "abc");
        compare("a%20bc", "a bc");
        compare("a%2Fbc", "a/bc");
        compare("%EA%B0%80%EB%82%98%EB%8B%A4", "가나다");
    }

    #[test]
    fn decode_percent_bad() {
        assert_none("%");
        assert_none("%2");
        assert_none("%FF");
    }
}
