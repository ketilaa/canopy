package com.example.greeting;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class GreeterHiddenTest {
    @Test
    void trimsTheNameBeforeGreeting() {
        assertEquals("Hello, Ada!", new Greeter().greet("  ada "));
    }

    @Test
    void stillGreetsAnUntrimmedNameAsBefore() {
        assertEquals("Hello, Grace!", new Greeter().greet("Grace"));
    }

    @Test
    void aBlankNameStillBecomesGuest() {
        assertEquals("Hello, guest!", new Greeter().greet("   "));
    }

    @Test
    void aNullNameStillBecomesGuest() {
        assertEquals("Hello, guest!", new Greeter().greet(null));
    }
}
