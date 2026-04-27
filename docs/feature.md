# Чего не хватает до «полноценного» веб-фреймворка (ресерч)

Документ фиксирует **разрыв** между текущим OxyRoute (RSGI + Rust hot path, см. [index](index.md)) и ожиданиями от **широкого** HTTP-фреймворка уровня FastAPI / Starlette / Django REST. Термин «полноценный» здесь означает **покрытие типичного продакшн-API и DX**, а не обязательность всех пунктов для твоего позиционирования.

**Следующий релиз:** в репозитории и в GitHub milestone целим **0.2.0** / [**v0.2.0**](https://github.com/QueryaHub/OxyRoute/milestone/1) (не 0.3.0); см. [PRIORITIES](../.github/ISSUE_BACKLOG/PRIORITIES.md).

---

## Уже есть (кратко)

- Маршрутизация по методам и путям (`matchit`), path/query/json/body, **405 / Allow**, **HEAD** на GET, **OPTIONS**.
- **JWT** (HS/RS/… через `jsonwebtoken`), iss/aud/leeway, cookie, **зависимости** с `request`, линейный порядок, `freeze`.
- **Response** / dict с заголовками и cookies, частичный **OpenAPI**, Pydantic/schema для тела.
- Один **pre-route middleware** (`set_middleware`).
- CI, PyPI, E2E Granian RSGI.

Ниже — то, чего **нет** или что **слабо** относительно «больших» фреймворков.

---

## 1. Протокол и транспорт

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **WebSockets** | Не реализованы | Поддержка ASGI WS была удалена в v0.3.0; native RSGI WebSocket — следующая работа. |
| **SSE / длинный стрим ответа** | Частично | Есть `send_sse` (см. [sse.md](sse.md)); инкрементальный стрим использует `response_stream` Granian RSGI. |
| **HTTP/2 push, trailers** | Не в фокусе | Обычно на стороне сервера; фреймворк редко экспонирует. |
| **ASGI совместимость** | Удалена в v0.3.0 | Поддерживается только RSGI (Granian `--interface rsgi`). |

---

## 2. Запрос и тело

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **`multipart/form-data`** | Частично | `read_form_body=True`, инжект `form` / `files` (в памяти); см. [handlers](handlers.md). [#47](https://github.com/QueryaHub/OxyRoute/issues/47). |
| **`application/x-www-form-urlencoded` (body)** | Частично | То же — `form`; нет streaming для очень больших тел. |
| **Streaming request body** | Ограничено | Тело читается в буфер для JSON/сырых байт; большие upload без полного чтения — отдельная работа. |
| **Валидация на уровне фреймворка** | Частично | Pydantic удобнее вручную; нет единого `Body()` как в FastAPI для всех типов контента. |

---

## 3. Маршрутизация и композиция

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **Sub-routers / `include_router`** | Частично | `APIRouter`, `App.include_router` — [routing](routing.md), [#46](https://github.com/QueryaHub/OxyRoute/issues/46). |
| **`Mount` / static files** | Нет | Отдача `/static` из каталога — обычно отдельный слой (nginx) или Starlette StaticFiles. |
| **Host-based routing** | Нет | |
| **Глобальные exception handlers** | Частично | `HTTPException` → нужный статус/JSON (см. [handlers](handlers.md)); **нет** `register_exception_handler` / иерархии как в FastAPI — [#48](https://github.com/QueryaHub/OxyRoute/issues/48). |
| **Middleware-цепочка** | Один хук | Нет порядка нескольких middleware и `on_request` / `on_response` как в ASGI-стеке. |

---

## 4. Безопасность (кроме JWT)

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **CORS** | `CORSConfig` + `apply_cors` / `set_cors` | Preflight и заголовки на ответах; см. [cors.md](cors.md). |
| **CSRF** | `CSRFConfig` / `apply_csrf` / `csrf_layer` | См. [csrf.md](csrf.md); double-submit, для Bearer-only API чаще не нужен. |
| **Rate limiting** | Нет | |
| **Security headers** (HSTS, CSP, …) | `SecurityHeadersConfig` + `set_security_headers` | [security-headers.md](security-headers.md); merge не перезаписывает уже заданные в `Response` имена. |
| **JWKS / ротация ключей** | Частично | В бэклоге [#8](https://github.com/QueryaHub/OxyRoute/issues/8). |

---

## 5. Состояние приложения и сессии

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **Встроенный `app.state` / lifespan** | Частично | `App.state` (`SimpleNamespace`), хуки `__rsgi_init__` / `__rsgi_del__`, пример `examples/rsgi_lifespan_app.py` — закрыто в [#18](https://github.com/QueryaHub/OxyRoute/issues/18). |
| **Сессии (signed cookie, server-side)** | Нет | |
| **Кэш глобальных настроек** | Нет | Нет первого класса для config. |

---

## 6. Разработка и тестирование

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **TestClient** (как Starlette/FastAPI) | Нет | Тесты через httpx + ASGI/реальный Granian; нет единого обёрточного клиента из коробки. |
| **CLI** (`oxyroute dev`, scaffold) | Нет | |
| **OpenAPI генерация клиентов** | Частично | Документ есть; не обещается полная совместимость со всеми генераторами. |
| **Background tasks** | Нет | Нет `BackgroundTasks` после ответа; только внешний воркер/очередь. |

---

## 7. Наблюдаемость

| Тема | Зазор | Комментарий |
|------|--------|-------------|
| **Structured access log** | Зависит от Granian | Нет формата «как у фреймворка». |
| **Metrics (Prometheus)** | Нет | |
| **Tracing (OpenTelemetry)** | Нет | |

---

## 8. Экосистема и позиционирование

- **ORM / миграции** — не часть HTTP-фреймворка; обычно SQLAlchemy + Alembic отдельно.
- **Шаблоны HTML (Jinja)** — нет; для API не критично.
- **GraphQL / gRPC** — отдельные стеки.

OxyRoute осознанно **уже** в нише: **быстрый маршрут + JSON + JWT в Rust**. «Полноценность» для многих команд = **multipart, WebSocket, цепочка middleware, суброутеры, TestClient** — это хороший кандидат на дорожную карту, если цель — конкурировать с FastAPI по удобству, а не только по RSGI.

---

## Приоритизация (рекомендация)

1. **Состояние и lifecycle** [#18](https://github.com/QueryaHub/OxyRoute/issues/18) — без этого сложно делить конфиг по воркерам.  
2. **ASGI надёжность** [#17](https://github.com/QueryaHub/OxyRoute/issues/17) — если ASGI остаётся равноправным входом.  
3. **Перф роутера** [#4](https://github.com/QueryaHub/OxyRoute/issues/4) — при росте нагрузки.  
4. **Sub-routers или префиксы** — резко повышают пригодность для крупных приложений.  
5. **Multipart + form body** — если не только JSON API.  
6. **Exception handlers (глобальные)** — быстрые победы на Python-стороне без ломки RSGI. **CORS** — см. [cors.md](cors.md) / [#49](https://github.com/QueryaHub/OxyRoute/issues/49).

## Связанные GitHub-issues (milestone v0.2.0)

**Композиция и тело / ошибки / CORS**

- [#46](https://github.com/QueryaHub/OxyRoute/issues/46) — sub-routers ([`21.md`](../.github/ISSUE_BACKLOG/bodies/21.md))  
- [#47](https://github.com/QueryaHub/OxyRoute/issues/47) — multipart / urlencoded ([`22.md`](../.github/ISSUE_BACKLOG/bodies/22.md))  
- [#48](https://github.com/QueryaHub/OxyRoute/issues/48) — **глобальные исключения** / `HTTPException` ([`23.md`](../.github/ISSUE_BACKLOG/bodies/23.md))  

**Протокол и безопасность**

- [#50](https://github.com/QueryaHub/OxyRoute/issues/50) — HTTP/2 (док/Granian) ([`25.md`](../.github/ISSUE_BACKLOG/bodies/25.md))  
- [#51](https://github.com/QueryaHub/OxyRoute/issues/51) — SSE ([`26.md`](../.github/ISSUE_BACKLOG/bodies/26.md))  
- [#52](https://github.com/QueryaHub/OxyRoute/issues/52) — WebSocket ([`27.md`](../.github/ISSUE_BACKLOG/bodies/27.md))  
- [#53](https://github.com/QueryaHub/OxyRoute/issues/53) — CSRF ([`28.md`](../.github/ISSUE_BACKLOG/bodies/28.md), [csrf.md](csrf.md)) — **сделано**  
- [#54](https://github.com/QueryaHub/OxyRoute/issues/54) — security headers ([`29.md`](../.github/ISSUE_BACKLOG/bodies/29.md), [security-headers.md](security-headers.md)) — **сделано**  

---

[← Documentation index](index.md)
