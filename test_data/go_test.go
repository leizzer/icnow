package main

import (
	"fmt"
	"net/http"
	"github.com/gin-gonic/gin"
)

type User struct {
	ID   int    `json:"id"`
	Name string `json:"name"`
}

type UserRepository interface {
	GetUser(id int) (*User, error)
}

func (u *User) GetName() string {
	return u.Name
}

func main() {
	r := gin.Default()
	r.GET("/users/:id", func(c *gin.Context) {
		c.JSON(http.StatusOK, gin.H{"id": 1, "name": "John Doe"})
	})
	r.Run() // listen and serve on 0.0.0.0:8080 (for windows "localhost:8080")
}
