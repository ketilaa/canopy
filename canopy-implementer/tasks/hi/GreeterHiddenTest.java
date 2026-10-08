package com.example.demo;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.Test;

class GreeterHiddenTest {
    @Test
    void greetsWithHi() {
        assertEquals("Hi, Ada!", new Greeter().greet("Ada"));
    }

    @Test
    void greetsAnotherNameWithHi() {
        assertEquals("Hi, Grace!", new Greeter().greet("Grace"));
    }

    @Test
    void stillRejectsABlankName() {
        assertThrows(IllegalArgumentException.class, () -> new Greeter().greet("   "));
    }
}
