"""Built-in OpenAPI docs UIs (Scalar / Swagger UI) loaded from CDN."""

from __future__ import annotations

from html import escape

__all__ = ["docs_html", "normalize_docs_ui"]

_VALID = frozenset({"scalar", "swagger"})


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
) -> str:
    """Return HTML for Scalar or Swagger UI pointing at ``openapi_url``."""
    ui_n = normalize_docs_ui(ui)
    if ui_n is None:
        raise ValueError("docs_ui is required")
    safe_title = escape(title)
    safe_url = escape(openapi_url, quote=True)
    if ui_n == "scalar":
        return _scalar_html(safe_title, safe_url)
    return _swagger_html(safe_title, safe_url)


def _scalar_html(title: str, openapi_url: str) -> str:
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
    src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
</body>
</html>
"""


def _swagger_html(title: str, openapi_url: str) -> str:
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <title>{title} — API docs</title>
  <link rel="stylesheet"
    href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css"/>
  <style>body {{ margin: 0; }}</style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
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
