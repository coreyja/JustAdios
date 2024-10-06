use maud::{html, Markup, Render};

pub struct Footer(Markup);

impl Default for Footer {
    fn default() -> Self {
        Footer(html! {
            p {
                "Footer"
            }
        })
    }
}

impl Render for Footer {
    fn render(&self) -> Markup {
        self.0.render()
    }
}
