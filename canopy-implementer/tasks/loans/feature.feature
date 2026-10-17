Feature: Library loans

  Scenario: A book is lent for 14 days
    When member "M-1" borrows the book "978-0" on 2026-03-01
    Then the loan is due on 2026-03-15

  Scenario: A member can have at most 3 active loans
    Given member "M-1" has 3 active loans
    When the member borrows a fourth book
    Then the loan is rejected with the message "Loan limit reached"

  Scenario: A book that is on loan cannot be borrowed
    Given the book "978-0" is on loan to member "M-1"
    When member "M-2" borrows the book "978-0"
    Then the loan is rejected with the message "Book is already on loan"

  Scenario Outline: A late return costs 0.50 per day after the due date
    Given the book "978-0" was borrowed on 2026-03-01 and is due on 2026-03-15
    When it is returned on <returned>
    Then the late fee is <fee>

    Examples:
      | returned   | fee  |
      | 2026-03-15 | 0.00 |
      | 2026-03-16 | 0.50 |
      | 2026-03-25 | 5.00 |

  Scenario: A returned book can be borrowed again and frees the member's loan slot
    Given member "M-1" has 3 active loans
    When one of the books is returned
    Then member "M-1" can borrow a book again

  Scenario: Returning a book that is not on loan is rejected
    When the book "978-9" is returned
    Then the return is rejected with the message "Book is not on loan"
