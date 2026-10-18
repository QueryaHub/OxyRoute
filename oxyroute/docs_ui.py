"""Built-in OpenAPI docs UIs (Scalar / Swagger UI) loaded from CDN with pinned versions and SRI."""

from __future__ import annotations

from html import escape

__all__ = [
    "SCALAR_JS_SRI",
    "SCALAR_JS_URL",
    "SCALAR_VERSION",
    "SWAGGER_CSS_SRI",
    "SWAGGER_CSS_URL",
    "SWAGGER_JS_SRI",
    "SWAGGER_JS_URL",
    "SWAGGER_UI_VERSION",
    "docs_html",
    "normalize_docs_ui",
]

_VALID = frozenset({"scalar", "swagger"})

# Pinned versions and Subresource Integrity (SRI) hashes for public CDN assets
SCALAR_VERSION = "1.75.0"
SCALAR_JS_URL = f"https://cdn.jsdelivr.net/npm/@scalar/api-reference@{SCALAR_VERSION}/dist/browser/standalone.js"
SCALAR_JS_SRI = "sha384-xgelKlDcc+BYp0O+ZYzFesLnWCgGpEbXVVZWNQT6WyZYygN01tbEMygnjPXb5OPI"

SWAGGER_UI_VERSION = "5.33.3"
SWAGGER_CSS_URL = (
    f"https://cdn.jsdelivr.net/npm/swagger-ui-dist@{SWAGGER_UI_VERSION}/swagger-ui.css"
)
SWAGGER_CSS_SRI = "sha384-Ov4/wv3j2bmct8cDc5X4ngJZohVPzEmc6uDPH8WeljUxO5vtoykvMEfbu9Vh6RaW"
SWAGGER_JS_URL = (
    f"https://cdn.jsdelivr.net/npm/swagger-ui-dist@{SWAGGER_UI_VERSION}/swagger-ui-bundle.js"
)
SWAGGER_JS_SRI = "sha384-h00llMzD1afyNow1pDmnZKlHsUZgp9KE9HXLFXF2CvbwsBY0+WBO4f75SfGwvM4K"


def normalize_docs_ui(ui: str | None) -> str | None:
    if ui is None:
        return None
    v = ui.strip().lower()
    if v not in _VALID:
        raise ValueError(f"docs_ui must be one of {sorted(_VALID)} or None, got {ui!r}")
    return v


def docs_html(
    *,
    ui: str,
    title: str,
    openapi_url: str = "/openapi.json",
    scalar_js_url: str = SCALAR_JS_URL,
    scalar_js_sri: str | None = SCALAR_JS_SRI,
    swagger_css_url: str = SWAGGER_CSS_URL,
    swagger_css_sri: str | None = SWAGGER_CSS_SRI,
    swagger_js_url: str = SWAGGER_JS_URL,
    swagger_js_sri: str | None = SWAGGER_JS_SRI,
) -> str:
    """Return HTML for Scalar or Swagger UI pointing at ``openapi_url``."""
    ui_n = normalize_docs_ui(ui)
    if ui_n is None:
        raise ValueError("docs_ui is required")
    safe_title = escape(title)
    safe_url = escape(openapi_url, quote=True)
    if ui_n == "scalar":
        return _scalar_html(
            title=safe_title,
            openapi_url=safe_url,
            js_url=scalar_js_url,
            js_sri=scalar_js_sri,
        )
    return _swagger_html(
        title=safe_title,
        openapi_url=safe_url,
        css_url=swagger_css_url,
        css_sri=swagger_css_sri,
        js_url=swagger_js_url,
        js_sri=swagger_js_sri,
    )


def _scalar_html(
    title: str,
    openapi_url: str,
    js_url: str,
    js_sri: str | None,
) -> str:
    sri_attr = (
        f' integrity="{escape(js_sri, quote=True)}" crossorigin="anonymous"' if js_sri else ""
    )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <title>{title} — API docs</title>
  <style>body {{ margin: 0; }}</style>
</head>
<body>
  <script
    id="api-reference"
    data-url="{openapi_url}"
    src="{escape(js_url, quote=True)}"{sri_attr}></script>
</body>
</html>
"""


def _swagger_html(
    title: str,
    openapi_url: str,
    css_url: str,
    css_sri: str | None,
    js_url: str,
    js_sri: str | None,
) -> str:
    css_sri_attr = (
        f' integrity="{escape(css_sri, quote=True)}" crossorigin="anonymous"' if css_sri else ""
    )
    js_sri_attr = (
        f' integrity="{escape(js_sri, quote=True)}" crossorigin="anonymous"' if js_sri else ""
    )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <title>{title} — API docs</title>
  <link rel="stylesheet"
    href="{escape(css_url, quote=True)}"{css_sri_attr}/>
  <style>body {{ margin: 0; }}</style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="{escape(js_url, quote=True)}"{js_sri_attr}></script>
  <script>
    window.ui = SwaggerUIBundle({{
      url: "{openapi_url}",
      dom_id: "#swagger-ui",
      presets: [SwaggerUIBundle.presets.apis, SwaggerUIBundle.SwaggerUIStandalonePreset],
      layout: "BaseLayout"
    }});
  </script>
</body>
</html>
"""
