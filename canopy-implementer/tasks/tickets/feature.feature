Feature: Single-journey tickets

  Scenario: An issued ticket is issued
    When a ticket "T-1" is issued at 2026-03-01 10:00
    Then the ticket "T-1" has the status ISSUED

  Scenario Outline: A ticket is valid for 90 minutes from when it was issued
    Given a ticket "T-1" issued at 2026-03-01 10:00
    When it is validated at <time>
    Then the outcome is <outcome>

    Examples:
      | time             | outcome                            |
      | 2026-03-01 10:30 | the ticket is VALIDATED            |
      | 2026-03-01 11:31 | rejected with "Ticket has expired" |

  Scenario: A validated ticket cannot be validated again
    Given a ticket "T-1" issued at 2026-03-01 10:00 and validated at 10:10
    When it is validated at 10:20
    Then the validation is rejected with the message "Ticket is already validated"

  Scenario: A ticket that is not validated can be cancelled
    Given a ticket "T-1" issued at 2026-03-01 10:00
    When it is cancelled
    Then the ticket has the status CANCELLED

  Scenario: A cancelled ticket cannot be validated
    Given a ticket "T-1" issued at 2026-03-01 10:00 and cancelled
    When it is validated at 10:10
    Then the validation is rejected with the message "Ticket is cancelled"

  Scenario: A validated ticket cannot be cancelled
    Given a ticket "T-1" issued at 2026-03-01 10:00 and validated at 10:10
    When it is cancelled
    Then the cancellation is rejected with the message "A validated ticket cannot be cancelled"

  Scenario: An unknown ticket cannot be validated or cancelled
    When the ticket "T-9" is validated or cancelled
    Then the request is rejected with the message "Unknown ticket"
