use maud::{html, Markup, Render};

pub struct Footer(Markup);

impl Default for Footer {
    fn default() -> Self {
        Footer(html! {
            footer class="bg-gray-800 text-white p-4 mt-auto" {
                p {
                  "© Copyright 2024 Corey Alexander"
                }
            }
        })
    }
}

impl Render for Footer {
    fn render(&self) -> Markup {
        self.0.render()
    }
}
