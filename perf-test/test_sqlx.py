from oxyroute import App

class DBApp(App):
    def __rsgi_init__(self, loop, *args, **kwargs):
        async def init():
            try:
                print("Setting up DB...")
                await self.setup_database("postgresql://postgres:postgres@127.0.0.1:5433/postgres", max_connections=10)
                print("DB Setup Complete!")
            except Exception as e:
                print("DB Setup Error:", e)
        loop.create_task(init())

app = DBApp(title="OxyRoute DB Test")

@app.get("/")
def index():
    return "ok"
