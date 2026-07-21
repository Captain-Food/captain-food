//! The SDUI renderer skeleton (split 1/4 of #21).
//!
//! Every renderable node is keyed by a GENERATED [`ComponentKind`] (the spec allowlist), so a screen
//! can only reference components declared in `customer_screens.yaml#/component_registry`. This first
//! slice renders ONE static screen to an HTML string server-side; the Leptos SSR/hydration runtime and
//! the live resolvers/actions arrive in later splits. The `data-hydrate` root marker is where the
//! client hydration entry will attach.

use crate::generated::registry::ComponentKind;

/// A minimal static SDUI node: a registered component kind plus its optional literal text. The live
/// renderer will carry props/children/bindings; this skeleton proves the registry dispatch seam.
#[derive(Debug, Clone)]
pub struct StaticNode {
    pub kind: ComponentKind,
    pub text: Option<&'static str>,
}

impl StaticNode {
    pub fn new(kind: ComponentKind, text: &'static str) -> Self {
        Self { kind, text: Some(text) }
    }
}

/// Render one node to HTML, dispatching on its [`ComponentKind`]. The tag/shape is a skeleton
/// stand-in (the real per-component Leptos views land next); the invariant proven here is that
/// rendering is driven by the generated allowlist, each node tagged with its spec `type`.
fn render_node(node: &StaticNode) -> String {
    let ty = node.kind.as_str();
    let body = esc(node.text.unwrap_or(""));
    match node.kind {
        ComponentKind::PageHeader => format!("<header data-c=\"{ty}\"><h1>{body}</h1></header>"),
        ComponentKind::Text => format!("<p data-c=\"{ty}\">{body}</p>"),
        ComponentKind::CtaBanner => format!("<aside data-c=\"{ty}\" class=\"cta\">{body}</aside>"),
        // Skeleton fallback: any other registered kind renders as a tagged block until its real view lands.
        _ => format!("<div data-c=\"{ty}\">{body}</div>"),
    }
}

/// Escape the five HTML-significant characters (skeleton; the live renderer routes text through
/// leptos_i18n + typed props).
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A single static screen (a minimal subset of the `home` chrome) rendered server-side to a full HTML
/// document. Everything it renders is dispatched through the generated component registry.
pub fn render_static_home() -> String {
    let nodes = [
        StaticNode::new(ComponentKind::PageHeader, "Captain.Food"),
        StaticNode::new(ComponentKind::Text, "Order from independent restaurants in Tours."),
        StaticNode::new(ComponentKind::CtaBanner, "Run a restaurant? Partner with us."),
    ];
    let inner: String = nodes.iter().map(render_node).collect();
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>Captain.Food</title></head>\
<body><main id=\"app\" data-hydrate=\"home\">{inner}</main></body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_home_renders_registered_components_only() {
        let html = render_static_home();
        assert!(html.contains("<title>Captain.Food</title>"));
        assert!(html.contains("data-hydrate=\"home\""));
        // Every rendered component tag is a member of the generated allowlist.
        assert!(html.contains("data-c=\"page_header\""));
        assert!(html.contains("data-c=\"text\""));
        assert!(html.contains("data-c=\"cta_banner\""));
    }

    #[test]
    fn registry_allowlist_round_trips() {
        for kind in ComponentKind::ALL {
            assert_eq!(ComponentKind::from_type(kind.as_str()), Some(*kind));
        }
        assert_eq!(ComponentKind::from_type("not_a_component"), None);
    }
}
