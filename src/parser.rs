use crate::get;

use std::io;
use core::fmt;

const NOVEL_TITLE_CONTAINER: &str = ".novel-item";
const CHAPTER_LIST_CONTAINER: &str = ".chapter-list";

use kuchiki::traits::TendrilSink;

fn fetch_chapter_indx(title: &str, page: u32) -> Result<Option<kuchiki::NodeRef>, String> {
    let url = format!("https://www.webnovelpub.com/novel/{}/chapters/page-{}", title, page);
    println!("!>>>Next chapter index={}", url);
    let resp = match get(&url) {
        Ok(resp) => if resp.status() != 200 {
            return Err(format!("Request to chapter index got unexpected result code: {}", resp.status()));
        } else {
            resp
        },
        Err(ureq::Error::Status(404, _)) => {
            return Ok(None);
        },
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("Request to chapter index failed with code: {}", code));
        },
        Err(ureq::Error::Transport(_)) => {
            return Err("www.webnovelpub.com is unreachable".to_owned());
        },
    };

    match resp.into_string() {
        Ok(page) => Ok(Some(kuchiki::parse_html().from_utf8().one(page.as_bytes()))),
        Err(error) => Err(format!("Request to chapter index got invalid body: {}", error)),
    }
}

pub enum WriteError {
    Http(String),
    Protocol(String),
    File(io::Error),
}

impl fmt::Display for WriteError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Http(error) => fmt.write_str(error.as_str()),
            Self::Protocol(error) => fmt.write_str(error.as_str()),
            Self::File(error) => fmt.write_fmt(format_args!("Write file error: {}", error)),
        }
    }
}

pub struct Chapter {
    pub title: String,
    pub url: String,
}

impl Chapter {
    pub fn write_chapter<W: io::Write>(&self, out: &mut W) -> Result<(), WriteError> {
        const WHITE_SPACE: &[char] = &[' ', '\t', '\n', '　'];
        const NOVEL_BODY: &str = "#chapter-container";

        let resp = match get(&format!("https://www.webnovelpub.com{}", self.url)) {
            Ok(resp) => if resp.status() != 200 {
                return Err(WriteError::Http(format!("{}: Reques got unexpected result code: {}", self.url, resp.status())));
            } else {
                resp
            },
            Err(ureq::Error::Status(code, _)) => {
                return Err(WriteError::Http(format!("{}: Request failed with code: {}", self.url, code)));
            },
            Err(ureq::Error::Transport(_)) => {
                return Err(WriteError::Http("www.webnovelpub.com is unreachable".to_owned()));
            },
        };

        let document = match resp.into_string() {
            Ok(page) => kuchiki::parse_html().from_utf8().one(page.as_bytes()),
            Err(error) => return Err(WriteError::Http(format!("{}: Request to chapter index got invalid body: {}", self.url, error))),
        };

        let novel_text = match document.select_first(NOVEL_BODY) {
            Ok(node) => node,
            Err(_) => return Err(WriteError::Protocol(format!("{}: Unable to find body {NOVEL_BODY}", self.url))),
        };
        let novel_text = novel_text.as_node();

        if let Err(error) = out.write_fmt(format_args!("## {}\n\n", self.title)) {
            return Err(WriteError::File(error));
        }

        for child in novel_text.children() {
            if let Some(element) = child.into_element_ref() {
                //html5ever uses some retarded string cache that requires to manually deref it to
                //get string
                use core::ops::Deref;

                if element.name.local.deref().eq_ignore_ascii_case("p") {
                    for node in element.as_node().children() {
                        if let Some(text) = node.as_text() {
                            let text = text.borrow();
                            let text = text.trim_matches(WHITE_SPACE);

                            if !text.is_empty() {
                                if let Err(error) = out.write_all(text.as_bytes()) {
                                    return Err(WriteError::File(error));
                                }
                            }
                        } else if let Some(element) = node.into_element_ref() {
                            if element.name.local.deref().eq_ignore_ascii_case("em") {
                                for node in element.as_node().children() {
                                    if let Some(text) = node.as_text() {
                                        let text = text.borrow();
                                        let text = text.trim_matches(WHITE_SPACE);

                                        if !text.is_empty() {
                                            if let Err(error) = out.write_all(b" *") {
                                                return Err(WriteError::File(error));
                                            }
                                            if let Err(error) = out.write_all(text.as_bytes()) {
                                                return Err(WriteError::File(error));
                                            }
                                            if let Err(error) = out.write_all(b"* ") {
                                                return Err(WriteError::File(error));
                                            }
                                        }

                                    }
                                }
                            } else if element.name.local.deref().eq_ignore_ascii_case("strong") {
                                for node in element.as_node().children() {
                                    if let Some(text) = node.as_text() {
                                        let text = text.borrow();
                                        let text = text.trim_matches(WHITE_SPACE);

                                        if !text.is_empty() {
                                            if let Err(error) = out.write_all(b" **") {
                                                return Err(WriteError::File(error));
                                            }
                                            if let Err(error) = out.write_all(text.as_bytes()) {
                                                return Err(WriteError::File(error));
                                            }
                                            if let Err(error) = out.write_all(b"** ") {
                                                return Err(WriteError::File(error));
                                            }
                                        }
                                    }
                                }
                            }
                        } //node.as_element
                    } //element.as_node().children()

                    if let Err(error) = out.write_all(b"\n\n") {
                        return Err(WriteError::File(error));
                    }
                }
            }
        }

        Ok(())
    }
}

pub struct ChapterList {
    pub iter: ChapterListIter,
    pub proper_title: String,
}

pub struct ChapterListIter {
    pub title: String,
    page_idx: u32,
    current_page: kuchiki::NodeRef,
    current_chapters: kuchiki::iter::Siblings,
}

impl ChapterList {
    pub fn new(title: String) -> Result<Self, String> {
        let page_idx = 1;
        let current_page = match fetch_chapter_indx(&title, page_idx)? {
            Some(page) => page,
            None => return Err("Unable fetch a first page of chapter list".to_owned()),
        };

        let container = match current_page.select_first(NOVEL_TITLE_CONTAINER) {
            Ok(node) => node,
            Err(_) => return Err(format!("Unable to find novel title container class '{NOVEL_TITLE_CONTAINER}'")),
        };
        let container = container.as_node();
        let proper_title = match container.select_first("a") {
            Ok(node) => match node.as_node().as_element() {
                Some(element) => {
                    let title = kuchiki::ExpandedName::new("", "title");
                    element.attributes.borrow().map.get(&title).map(|attr| attr.value.clone()).unwrap_or_else(|| "UNKNOWN".to_owned())
                },
                None => {
                    return Err("Invalid element <a> with novel title".to_owned());
                }
            },
            Err(_) => return Err("Unable to find title in <a>".to_owned()),
        };

        let current_chapters = match current_page.select_first(CHAPTER_LIST_CONTAINER) {
            Ok(node) => node.as_node().children(),
            Err(_) => return Err(format!("Unable to find chapter list container class '{CHAPTER_LIST_CONTAINER}'")),
        };

        Ok(Self {
            proper_title,
            iter: ChapterListIter {
                title,
                page_idx,
                current_page,
                current_chapters,
            }
        })
    }
}

impl Iterator for ChapterListIter {
    type Item = Chapter;

    fn next(&mut self) -> Option<Self::Item> {
        for child in &mut self.current_chapters {
            if let Some(element) = child.into_element_ref() {
                for child in element.as_node().children() {
                    if let Some(link) = child.into_element_ref() {
                        let title = kuchiki::ExpandedName::new("", "title");
                        let title = match link.attributes.borrow().map.get(&title) {
                            Some(attr) => attr.value.clone(),
                            None => {
                                eprint!("chapter link is missing title");
                                return None;
                            }
                        };

                        let href = kuchiki::ExpandedName::new("", "href");
                        let url = match link.attributes.borrow().map.get(&href) {
                            Some(attr) => attr.value.clone(),
                            None => {
                                eprint!("chapter '{title}' link is missing href");
                                return None;
                            }
                        };

                        return Some(Chapter {
                            title,
                            url,
                        })
                    }
                }
            }
        }
        //no more chapters on current_page, so let's check next page
        self.page_idx += 1;
        let new_page = match fetch_chapter_indx(&self.title, self.page_idx) {
            Ok(Some(page)) => page,
            //No more chapter lists, so we're finished
            Ok(None) => return None,
            Err(error) => {
                eprintln!("Unable to fetch chapter: {}", error);
                return None;
            }
        };

        self.current_chapters = match new_page.select_first(CHAPTER_LIST_CONTAINER) {
            Ok(node) => node.as_node().children(),
            Err(_) => {
                eprintln!("Unable to find chapter list container class '{CHAPTER_LIST_CONTAINER}'");
                return None;
            }
        };

        self.current_page = new_page;
        self.next()
    }
}
