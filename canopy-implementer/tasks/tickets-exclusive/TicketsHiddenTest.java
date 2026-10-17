package no.experis.tickets;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.time.LocalDateTime;
import org.junit.jupiter.api.Test;

/** The tickets feature against the fixed API of the tickets plans. The model never sees this file. */
class TicketsHiddenTest {
    private static final LocalDateTime ISSUED = LocalDateTime.of(2026, 3, 1, 10, 0);
    private final TicketRepository repository = new InMemoryTicketRepository();
    private final TicketService service = new TicketService(repository);

    private static LocalDateTime at(int hour, int minute) {
        return LocalDateTime.of(2026, 3, 1, hour, minute);
    }

    @Test
    void anIssuedTicketIsIssuedAndKept() {
        Ticket ticket = service.issue("T-1", ISSUED);
        assertEquals(TicketStatus.ISSUED, ticket.status());
        assertEquals("T-1", ticket.id());
        assertEquals(ISSUED, ticket.issuedAt());
        assertTrue(repository.findById("T-1").isPresent());
    }

    @Test
    void aTicketIsValidUntilTheNinetiethMinuteHasRunOut() {
        Ticket ticket = new Ticket("T-1", ISSUED, TicketStatus.ISSUED);
        assertEquals(TicketStatus.VALIDATED, ticket.validate(at(10, 30)).status());
        assertEquals(TicketStatus.VALIDATED, ticket.validate(at(11, 29)).status());
        for (LocalDateTime late : new LocalDateTime[] {at(11, 30), at(11, 31)}) {
            IllegalStateException e = assertThrows(IllegalStateException.class, () -> ticket.validate(late));
            assertEquals("Ticket has expired", e.getMessage());
        }
        assertEquals(TicketStatus.ISSUED, ticket.status());
    }

    @Test
    void aValidatedTicketCannotBeValidatedAgainOrCancelled() {
        Ticket validated = new Ticket("T-1", ISSUED, TicketStatus.ISSUED).validate(at(10, 10));
        IllegalStateException again = assertThrows(IllegalStateException.class, () -> validated.validate(at(10, 20)));
        assertEquals("Ticket is already validated", again.getMessage());
        IllegalStateException cancel = assertThrows(IllegalStateException.class, validated::cancel);
        assertEquals("A validated ticket cannot be cancelled", cancel.getMessage());
    }

    @Test
    void aTicketThatIsNotValidatedCanBeCancelledAndThenNotValidated() {
        Ticket ticket = new Ticket("T-1", ISSUED, TicketStatus.ISSUED);
        Ticket cancelled = ticket.cancel();
        assertEquals(TicketStatus.CANCELLED, cancelled.status());
        assertEquals(TicketStatus.ISSUED, ticket.status());
        IllegalStateException e = assertThrows(IllegalStateException.class, () -> cancelled.validate(at(10, 10)));
        assertEquals("Ticket is cancelled", e.getMessage());
    }

    @Test
    void theServiceValidatesAndCancelsWhatItHasIssuedAndRejectsWhatItDoesNot() {
        service.issue("T-1", ISSUED);
        service.issue("T-2", ISSUED);
        assertEquals(TicketStatus.VALIDATED, service.validate("T-1", at(10, 30)).status());
        assertEquals(TicketStatus.VALIDATED, repository.findById("T-1").orElseThrow().status());
        assertEquals(TicketStatus.CANCELLED, service.cancel("T-2").status());
        IllegalStateException again = assertThrows(IllegalStateException.class, () -> service.validate("T-1", at(10, 40)));
        assertEquals("Ticket is already validated", again.getMessage());
        IllegalArgumentException unknownValidate = assertThrows(IllegalArgumentException.class, () -> service.validate("T-9", at(10, 30)));
        assertEquals("Unknown ticket", unknownValidate.getMessage());
        IllegalArgumentException unknownCancel = assertThrows(IllegalArgumentException.class, () -> service.cancel("T-9"));
        assertEquals("Unknown ticket", unknownCancel.getMessage());
    }

    @Test
    void theRepositoryKeepsWhatWasSavedAndKnowsWhatItDoesNot() {
        assertTrue(repository.findById("T-1").isEmpty());
        repository.save(new Ticket("T-1", ISSUED, TicketStatus.ISSUED));
        assertEquals(TicketStatus.ISSUED, repository.findById("T-1").orElseThrow().status());
        repository.save(new Ticket("T-1", ISSUED, TicketStatus.CANCELLED));
        assertEquals(TicketStatus.CANCELLED, repository.findById("T-1").orElseThrow().status());
    }
}
