from django.db import models
from fastapi import FastAPI

app = FastAPI()

class User(models.Model):
    name = models.CharField(max_length=100)

@app.get("/users")
async def get_users():
    return [{"name": "John"}]

@admin.register(User)
class UserAdmin(admin.ModelAdmin):
    pass
