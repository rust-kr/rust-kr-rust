extern mod extra;
extern mod http;
extern mod mustache;

use std::io::net::ip::{SocketAddr, Ipv4Addr, Port};
use std::io::{Writer, File};
use std::io::fs::readdir;

use extra::getopts;
use extra::time;

use http::server::{Config, Server, Request, ResponseWriter};
use http::server::request::AbsolutePath;
use http::headers::content_type::MediaType;
use http::status;

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
                        let c1 = it.next().and_then(|n| { hex_to_u8(n) });
                        let c1 = match c1 {
                            None => return None,
                            Some(c) => c,
                        };
                        let c2 = it.next().and_then(|n| { hex_to_u8(n) });
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
        // TODO macro
        let map = [
            ("/static/", |remaining: &str, r: &Request, w: &mut ResponseWriter| {
                self.handle_static_file(remaining, r, w)
            }),
            ("/pages/", |remaining: &str, r: &Request, w: &mut ResponseWriter| {
                self.handle_page(remaining, r, w)
            }),
            ("/", |remaining: &str, r: &Request, w: &mut ResponseWriter| {
                self.handle_index_page(remaining, r, w)
            }),
        ];

        match r.request_uri {
            AbsolutePath(ref url) => {
                let url = url.as_slice();

                // ignore any bad urls
                let url = url.decode_percent();
                let url = match url {
                    None => {
                        self.show_bad_request(w);
                        return;
                    }
                    Some(url) => url,
                };

                for &(ref prefix, ref handler) in map.iter() {
                    if url.starts_with(*prefix) {
                        let remaining = url.slice_from(prefix.len());
                        (*handler)(remaining, r, w);
                        return;
                    }
                }

                // default handler
                self.show_not_found(w);
            }
            _ => {
                // TODO
                self.show_not_found(w);
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
        if !std::str::is_utf8(text) {
            return None;
        }
        let text = std::str::from_utf8(text);
        let md = markdown::Markdown(text);
        Some(format!("{}", md))
    }

    pub fn list_pages(&self) -> ~str {
        let dir = Path::new(self.doc_dir.clone());
        if !dir.exists() {
            return ~"No pages found";
        }

        let files = readdir(&dir);
        let mut pages = ~[];
        for file in files.iter() {
            if file.is_dir() {
                continue;
            }
            match file.as_str() {
                None => continue,
                Some(s) => {
                    if s.ends_with(".md") {
                        let pagename = file.filestem_str();
                        match pagename {
                            None => continue,
                            Some(pagename) => {
                                if self.is_bad_title(pagename) {
                                    continue;
                                }
                                pages.push(pagename.to_owned());
                            }
                        }
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

    fn show_not_found(&self, w: &mut ResponseWriter) {
        let ctx = Ctx {
            title: ~"Not Found",
            content: ~"헐",
        };
        self.show_template(w, &ctx, status::NotFound);
    }

    fn show_bad_request(&self, w: &mut ResponseWriter) {
        let ctx = Ctx {
            title: ~"Bad request",
            content: ~"헐",
        };
        self.show_template(w, &ctx, status::BadRequest);
    }

    fn show_template(&self, w: &mut ResponseWriter, ctx: &Ctx, status: status::Status) {
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
        w.status = status;

        w.write(output_b);
    }

    fn handle_index_page(&self, remaining: &str, r: &Request, w: &mut ResponseWriter) {
        if remaining.len() > 0 {
            self.show_not_found(w);
            return;
        }
        self.handle_page("index", r, w);
    }

    fn handle_page(&self, title: &str, _: &Request, w: &mut ResponseWriter) {
        let (title, content) = match title {
            "_pages" => ("모든 문서", self.list_pages()),
            _ => {
                let content = self.read_page(title);
                match content {
                    Some(content) => (title, content),
                    None => {
                        return self.show_not_found(w);
                    }
                }
            }
        };
        let ctx = Ctx {
            title: title.to_owned(),
            content: content,
        };
        self.show_template(w, &ctx, status::Ok);
    }

    fn handle_static_file(&self, loc: &str, _: &Request, w: &mut ResponseWriter) {
        let path = Path::new(format!("static/{}", loc));
        if !path.exists() {
            self.show_not_found(w);
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
