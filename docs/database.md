# Database Integration & DBQuery Security

OxyRoute integrates native database query execution via `sqlx` in Rust, avoiding Python-level ORM overhead on hot data paths while preserving strict SQL injection prevention guarantees.

---

## Native DBQuery & Parameter Binding

When executing queries through `DBQuery`, all arguments must be bound using positional placeholders (`$1`, `$2`, `$3`, ...).

### Safe Parameter Binding (Recommended)

Always pass arguments as a tuple or list in `DBQuery(query, args)`:

```python
from oxyroute import App, DBQuery, Depends

app = App()

def get_user_query(email: str) -> DBQuery:
    # SAFE: $1 is parameterized and bound by sqlx at the driver level
    return DBQuery(
        "SELECT id, username, email FROM users WHERE email = $1 AND is_active = $2",
        (email, True),
    )

@app.get("/users/by-email", dependencies=[("user_data", Depends(get_user_query))])
def get_user(user_data):
    return {"user": user_data}
```

---

## SQL Injection Prevention (Important)

> [!CAUTION]
> **NEVER** use Python f-strings, `%` formatting, or string concatenation (`+`) to insert user input into SQL queries. Doing so bypasses parameterization and introduces severe SQL injection vulnerabilities.

### Vulnerable Examples (DO NOT DO THIS)

```python
# ❌ VULNERABLE TO SQL INJECTION:
email = "admin' OR 1=1; --"
query = DBQuery(f"SELECT * FROM users WHERE email = '{email}'")

# ❌ ALSO VULNERABLE:
query = DBQuery("SELECT * FROM users WHERE email = '" + email + "'")
```

### Supported Parameter Types

The native Rust executor automatically maps and type-checks the following Python types into Postgres wire formats:

| Python Type | Postgres Type |
|---|---|
| `None` | `NULL` |
| `bool` | `BOOL` |
| `int` | `INT2`, `INT4`, `INT8` (auto-selected) |
| `float` | `FLOAT4`, `FLOAT8` |
| `str` | `TEXT`, `VARCHAR`, `CHAR` |
| `dict` / `list` | `JSON`, `JSONB` |

---

## Database Connection Pool Lifecycle

Configure the Postgres pool in `on_startup` or via `app.setup_database`:

```python
@app.on_startup
async def startup():
    await app.setup_database(
        "postgresql://user:password@localhost:5432/dbname",
        max_connections=20,
    )

@app.on_shutdown
async def shutdown():
    await app.close_database()
```
