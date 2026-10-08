package com.example.shop;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import org.junit.jupiter.api.Test;

class VatHiddenTest {
    private final Order small = new Order(List.of(new Item("Widget", 2, 2000)));
    private final Order large = new Order(List.of(new Item("Gadget", 10, 2000)));
    private final PriceCalculator calculator = new PriceCalculator();
    private final InvoiceFormatter formatter = new InvoiceFormatter();
    private final OrderService service = new OrderService();

    private static boolean hasLine(String invoice, String prefix, String amount) {
        return invoice.lines().anyMatch(l -> l.toLowerCase().startsWith(prefix) && l.contains(amount));
    }

    @Test
    void theTotalIncludesVat() {
        assertEquals(5000, calculator.totalCents(small));
    }

    @Test
    void vatIsTakenOnTheDiscountedAmount() {
        assertEquals(22500, calculator.totalCents(large));
    }

    @Test
    void theCustomerIsChargedTheTotalIncludingVat() {
        assertEquals(5000, service.amountToCharge(small));
        assertEquals(22500, service.amountToCharge(large));
    }

    @Test
    void theInvoiceShowsVatAsItsOwnLineAndATotalThatIncludesIt() {
        String invoice = formatter.format(small);
        assertTrue(hasLine(invoice, "vat", "10.00"), invoice);
        assertTrue(hasLine(invoice, "total", "50.00"), invoice);
    }

    @Test
    void theInvoiceStillShowsTheDiscountAndTakesVatOnTheDiscountedAmount() {
        String invoice = formatter.format(large);
        assertTrue(hasLine(invoice, "discount", "20.00"), invoice);
        assertTrue(hasLine(invoice, "vat", "45.00"), invoice);
        assertTrue(hasLine(invoice, "total", "225.00"), invoice);
    }
}
