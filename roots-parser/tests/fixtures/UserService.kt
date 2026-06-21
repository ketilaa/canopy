package com.example.users

import com.example.core.BaseService

class UserService : BaseService() {
    fun findUser(id: String): User? = null
    fun createUser(name: String): User = User(name)
}

object UserCache {
    fun get(id: String): User? = null
}

interface UserRepository {
    fun findById(id: String): User?
}
