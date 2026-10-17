package no.experis.loans;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.math.BigDecimal;
import java.time.LocalDate;
import org.junit.jupiter.api.Test;

/** The loans feature against the fixed API of the loans plans. The model never sees this file. */
class LoansHiddenTest {
    private static final LocalDate DAY = LocalDate.of(2026, 3, 1);

    private static void assertFee(String expected, LocalDate returned) {
        LoanService service = new LoanService();
        service.borrow("M-1", "978-0", DAY);
        BigDecimal fee = service.returnBook("978-0", returned);
        assertEquals(0, new BigDecimal(expected).compareTo(fee), "returned " + returned + " should cost " + expected + " but was " + fee);
    }

    @Test
    void aBookIsLentFor14Days() {
        Loan loan = new LoanService().borrow("M-1", "978-0", DAY);
        assertEquals(LocalDate.of(2026, 3, 15), loan.dueOn());
        assertEquals("M-1", loan.memberId());
        assertEquals("978-0", loan.isbn());
        assertEquals(DAY, loan.borrowedOn());
    }

    @Test
    void aMemberCanHaveThreeActiveLoansButNotFour() {
        LoanService service = new LoanService();
        service.borrow("M-1", "A", DAY);
        service.borrow("M-1", "B", DAY);
        service.borrow("M-1", "C", DAY);
        assertEquals(3, service.activeLoans("M-1"));
        IllegalStateException e = assertThrows(IllegalStateException.class, () -> service.borrow("M-1", "D", DAY));
        assertEquals("Loan limit reached", e.getMessage());
        assertEquals(3, service.activeLoans("M-1"));
    }

    @Test
    void theLimitIsPerMember() {
        LoanService service = new LoanService();
        service.borrow("M-1", "A", DAY);
        service.borrow("M-1", "B", DAY);
        service.borrow("M-1", "C", DAY);
        service.borrow("M-2", "D", DAY);
        assertEquals(1, service.activeLoans("M-2"));
    }

    @Test
    void aBookThatIsOnLoanCannotBeBorrowedByAnyone() {
        LoanService service = new LoanService();
        service.borrow("M-1", "978-0", DAY);
        IllegalStateException e = assertThrows(IllegalStateException.class, () -> service.borrow("M-2", "978-0", DAY));
        assertEquals("Book is already on loan", e.getMessage());
        assertEquals(0, service.activeLoans("M-2"));
    }

    @Test
    void aLateReturnCostsHalfAnAmountPerDayAfterTheDueDate() {
        assertFee("0.00", LocalDate.of(2026, 3, 10));
        assertFee("0.00", LocalDate.of(2026, 3, 15));
        assertFee("0.50", LocalDate.of(2026, 3, 16));
        assertFee("5.00", LocalDate.of(2026, 3, 25));
    }

    @Test
    void aReturnedBookCanBeBorrowedAgainAndFreesTheLoanSlot() {
        LoanService service = new LoanService();
        service.borrow("M-1", "A", DAY);
        service.borrow("M-1", "B", DAY);
        service.borrow("M-1", "C", DAY);
        service.returnBook("B", DAY.plusDays(3));
        assertEquals(2, service.activeLoans("M-1"));
        service.borrow("M-1", "D", DAY.plusDays(3));
        service.borrow("M-2", "B", DAY.plusDays(3));
        assertEquals(3, service.activeLoans("M-1"));
    }

    @Test
    void returningABookThatIsNotOnLoanIsRejected() {
        LoanService service = new LoanService();
        IllegalArgumentException e = assertThrows(IllegalArgumentException.class, () -> service.returnBook("978-9", DAY));
        assertEquals("Book is not on loan", e.getMessage());
        service.borrow("M-1", "978-0", DAY);
        service.returnBook("978-0", DAY);
        assertThrows(IllegalArgumentException.class, () -> service.returnBook("978-0", DAY));
    }

    @Test
    void aLoanKnowsItsLateFee() {
        Loan loan = Loan.start("M-1", "978-0", DAY);
        assertEquals(LocalDate.of(2026, 3, 15), loan.dueOn());
        assertEquals(0, new BigDecimal("2.50").compareTo(loan.lateFee(LocalDate.of(2026, 3, 20))));
        assertEquals(0, BigDecimal.ZERO.compareTo(loan.lateFee(LocalDate.of(2026, 3, 1))));
    }
}
