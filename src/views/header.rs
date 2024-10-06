use maud::{html, Markup, Render};

use crate::db::DBUser;

use super::Section;

pub struct Header {
    current_section: Section,
    current_user: Option<DBUser>,
}

impl Header {
    pub fn new(section: Section, current_user: Option<DBUser>) -> Self {
        Self {
            current_section: section,
            current_user,
        }
    }

    fn links(&self) -> Vec<HeaderLink> {
        vec![
            HeaderLink {
                href: "/",
                text: "Dashboard",
                section: Section::Dashboard,
            },
            HeaderLink {
                href: "/meetings",
                text: "Meetings",
                section: Section::Meetings,
            },
            HeaderLink {
                href: "/settings",
                text: "Settings",
                section: Section::Settings,
            },
        ]
    }

    fn desktop_links(&self) -> Vec<HeaderDesktopLink> {
        self.links()
            .into_iter()
            .map(|link| HeaderDesktopLink {
                href: link.href,
                text: link.text,
                section: link.section,
                current_section: self.current_section,
            })
            .collect()
    }

    fn mobile_links(&self) -> Vec<HeaderMobileLink> {
        self.links()
            .into_iter()
            .map(|link| HeaderMobileLink {
                href: link.href,
                text: link.text,
                section: link.section,
                current_section: self.current_section,
            })
            .collect()
    }
}

struct HeaderLink {
    href: &'static str,
    text: &'static str,
    section: Section,
}

struct HeaderDesktopLink {
    href: &'static str,
    text: &'static str,
    section: Section,
    current_section: Section,
}

impl Render for HeaderDesktopLink {
    fn render(&self) -> Markup {
        if self.section == self.current_section {
            html! {
              a."rounded-md bg-indigo-700 px-3 py-2 text-sm font-medium text-white" href=(self.href) aria-current="page" {
                (self.text)
              }
            }
        } else {
            html! {
              a."rounded-md px-3 py-2 text-sm font-medium text-white hover:bg-indigo-500 hover:bg-opacity-75" href=(self.href) {
                (self.text)
              }
            }
        }
    }
}

struct HeaderMobileLink {
    href: &'static str,
    text: &'static str,
    section: Section,
    current_section: Section,
}

impl Render for HeaderMobileLink {
    fn render(&self) -> Markup {
        if self.section == self.current_section {
            html! {
              a."block rounded-md bg-indigo-700 px-3 py-2 text-base font-medium text-white" href=(self.href) aria-current="page" {
                (self.text)
              }
            }
        } else {
            html! {
              a."block rounded-md px-3 py-2 text-base font-medium text-white hover:bg-indigo-500 hover:bg-opacity-75" href=(self.href) {
                (self.text)
              }
            }
        }
    }
}

impl Render for Header {
    fn render(&self) -> Markup {
        html! {
            div."bg-indigo-600 pb-32" {
                nav."border-b border-indigo-300 border-opacity-25 bg-indigo-600 lg:border-none" {
                    div."mx-auto max-w-7xl px-2 sm:px-4 lg:px-8" {
                        div."relative flex h-16 items-center justify-between lg:border-b lg:border-indigo-400 lg:border-opacity-25" {
                            div."flex items-center px-2 lg:px-0" {
                                div."flex-shrink-0" {
                                    a href="/" {
                                        img."block h-8 w-8" src="https://tailwindui.com/plus/img/logos/mark.svg?color=indigo&shade=300" alt="Just Adios" {}
                                    }
                                }
                                div."hidden lg:ml-10 lg:block" {
                                    div."flex space-x-4" {
                                      @for link in self.desktop_links() {
                                        (link)
                                      }
                                    }
                                }
                            }
                            div."hidden lg:ml-4 lg:block" {
                                div."flex items-center" {
                                    div."relative ml-3 flex-shrink-0" {
                                        div {
                                            button."relative flex rounded-full bg-indigo-600 text-sm text-white focus:outline-none focus:ring-2 focus:ring-white focus:ring-offset-2 focus:ring-offset-indigo-600" id="user-menu-button" type="button" aria-expanded="false" aria-haspopup="true" {
                                                span."absolute -inset-1.5" {}
                                                @if let Some(Some(pic_url)) = self.current_user.as_ref().map(|user| user.cached_zoom_pic_url()) {
                                                  img."h-8 w-8 rounded-full" src=(pic_url) alt="" {}
                                                } @else {
                                                  img."h-8 w-8 rounded-full" src="https://images.unsplash.com/photo-1472099645785-5658abf4ff4e?ixlib=rb-1.2.1&ixid=eyJhcHBfaWQiOjEyMDd9&auto=format&fit=facearea&facepad=2&w=256&h=256&q=80" alt="" {}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div."lg:hidden" id="mobile-menu" {
                        div."space-y-1 px-2 pb-3 pt-2" {
                            @for link in self.mobile_links() {
                                (link)
                            }
                        }
                        div."border-t border-indigo-700 pb-3 pt-4" {
                          @if let Some(user) = &self.current_user {
                            div."flex items-center px-5" {
                                @if let Some(pic_url) = user.cached_zoom_pic_url() {
                                  div."flex-shrink-0" {
                                      img."h-10 w-10 rounded-full" src=(pic_url) alt="" {}
                                  }
                                }
                                div."ml-3" {
                                    div."text-base font-medium text-white" {
                                      (user.display_name)
                                    }
                                }
                            }
                          }
                        }
                    }
                }
                header."py-10" {
                    div."mx-auto max-w-7xl px-4 sm:px-6 lg:px-8" {
                        h1."text-3xl font-bold tracking-tight text-white" {
                          @match self.current_section {
                            Section::Dashboard => "Dashboard",
                            Section::Meetings => "Meetings",
                            Section::Settings => "Settings",
                          }
                        }
                    }
                }
            }
        }
    }
}
