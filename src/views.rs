use axum::response::IntoResponse;
use footer::Footer;
use header::Header;
use maud::{html, Markup, Render};

use crate::db::DBUser;

mod footer;
mod header;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Section {
    Dashboard,
    Meetings,
    Settings,
}

pub struct PageContent(Markup);

impl Render for PageContent {
    fn render(&self) -> Markup {
        self.0.render()
    }
}

pub struct Page {
    pub header: Header,
    pub content: PageContent,
    pub footer: Footer,
}

impl Render for Page {
    fn render(&self) -> Markup {
        html! {
          html class="h-full bg-gray-100" {
            head {
              title { "Just Adios" }
              script src="https://cdn.tailwindcss.com" {}
            }
            body class="h-full" {
              div class="min-h-full flex flex-col" {
                (self.header.render())
                main."-mt-32" {
                    div."mx-auto max-w-7xl px-4 pb-12 sm:px-6 lg:px-8" {
                        div."rounded-lg bg-white px-5 py-6 shadow sm:px-6" {
                          (self.content.render())
                        }
                    }
                }
                (self.footer.render())
              }
            }
          }
        }
    }
}

fn page(content: Markup, section: Section, current_user: Option<DBUser>) -> Page {
    Page {
        header: Header::new(section, current_user),
        content: PageContent(content),
        footer: Footer::default(),
    }
}

impl IntoResponse for Page {
    fn into_response(self) -> axum::response::Response {
        self.render().into_response()
    }
}

impl Section {
    pub fn page(self, content: Markup, current_user: Option<DBUser>) -> Page {
        page(content, self, current_user)
    }
}
