# Spec Delta

## Purpose

PlatynUI Café is the demo application the user documentation teaches with: a café point-of-sale app that every reader can install and that behaves the same on Windows and Linux. Its screens, data and deliberate stumbling blocks are a contract the tutorials, guides and tested documentation examples rely on, so they stay stable once the documentation uses them.

## ADDED Requirements

### Requirement: Installation and launch
The demo SHALL be distributed as the pure-Python package `platynui-demo`. The package SHALL depend on nothing from PlatynUI, so a reader can install it on its own. It SHALL install a command `platynui-demo`, and `python -m platynui_demo` SHALL start the same application. It SHALL run on Windows and on Linux (X11, and Wayland under the PlatynUI compositor). It SHALL look the same on every platform, independent of the system theme. On Linux it SHALL enable its own accessibility bridge, so no environment variable is needed to make it visible to AT-SPI. It SHALL NOT override a value the user has already set.

#### Scenario: Installed command starts the app
- **GIVEN** `platynui-demo` was installed with `uv tool install platynui-demo`
- **WHEN** `platynui-demo` is run without arguments
- **THEN** a top-level window titled `PlatynUI Cafe - Register 1` opens and appears on the accessibility tree under an application node

#### Scenario: Visible to AT-SPI without extra setup
- **GIVEN** a Linux session with the AT-SPI bus running and no Qt accessibility variable set in the environment
- **WHEN** the demo is started
- **THEN** its main window and controls resolve through the AT-SPI provider (real provider only)

### Requirement: Command-line options
The demo SHALL accept the following options:

- `--lang en|de`: the display language, default `en`.
- `--register <1-9>`: the register number shown in the main window title, default `1`.
- `--login`: open the sign-in window before the register.
- `--splash`: show a splash window before the first window.
- `--delay-factor <number ≥ 0>`: multiplies every built-in delay, default `1`.
- `--version` and `--help`.

An unknown option or an invalid value SHALL print a usage message that names the offending argument, and the demo SHALL exit with a non-zero code without opening a window.

#### Scenario: Register number in the title
- **WHEN** the demo starts with `--register 2`
- **THEN** its main window is titled `PlatynUI Cafe - Register 2`

#### Scenario: Unknown option fails fast
- **WHEN** the demo starts with `--bogus`
- **THEN** it prints a usage message naming `--bogus`, exits with a non-zero code, and no window appears

#### Scenario: Invalid value fails fast
- **WHEN** the demo starts with `--delay-factor -1` or `--lang fr`
- **THEN** it prints a usage message naming the invalid value, exits with a non-zero code, and no window appears

### Requirement: Deterministic data and timing
Every start SHALL show the same seed data. The demo SHALL NOT read or write any persistent state: no settings file, no saved window geometry, no data file. The main window SHALL open at a fixed size. Seed data and prices (English formatting):

- **Products:**
  - Espresso 2.40, Cappuccino 3.00, Latte 3.60, Flat White 3.50, Americano 2.80, Mocha 3.90, Filter Coffee 2.50 (category Coffee).
  - Green Tea 2.60, Earl Grey 2.60, Chai Latte 3.80 (category Tea).
  - Croissant 2.50, Muffin 2.90, Cheesecake 3.90, Seasonal Special 4.20 (category Food, Sweet), and Bagel 3.40 (category Food, Savory).
- **Surcharges for drinks:** size S −0.20, M ±0, L +0.40. Oat or soy milk +0.30, extra shot +0.50, syrup +0.40.
- **Order history:** orders #1001 to #1040, taken by the baristas Mia, Jonas, Aylin and Luca, with payment Cash or Card and status Served or Refunded.
  - Exactly one order by Mia has the total 9.20: order #1017, Card, Served.
  - Order #1020 is Served.
  - Every barista has at least five orders, Jonas has at least two paid by Cash, and at least one order is Refunded.
- **Stock:** twelve items, listed alphabetically by name. Exactly three are below their minimum:
  - Chai syrup (on hand 1, minimum 3)
  - Croissant (on hand 4, minimum 10)
  - Oat milk (on hand 2, minimum 6)

New orders SHALL be numbered from #1041 upward. The built-in delays SHALL be the following, each multiplied by `--delay-factor`:

- card terminal: 3 s
- brewing one kitchen ticket: 5 s
- loading the daily report: 2 s
- splash: 2 s

#### Scenario: Same data on every start
- **GIVEN** the demo was started, an order was paid, and the demo was closed
- **WHEN** the demo is started again
- **THEN** the Orders table again ends with order #1040 and the next new order is #1041

#### Scenario: Delays scale with the delay factor
- **GIVEN** the demo was started with `--delay-factor 0`
- **WHEN** a card payment with a valid PIN is confirmed
- **THEN** the payment completes without the 3-second terminal wait

### Requirement: Stable identity of elements
Every interactive element and every element a documented example reads SHALL carry an accessible id. The id SHALL be English, kebab-case and the same in every language, and it SHALL surface as the common `@Id` on Windows and Linux. The accessible name of an element SHALL be its visible text. Window titles SHALL be the only name of a window. They SHALL use ASCII characters only (`PlatynUI Cafe`, a plain hyphen), so that locators can be typed on any keyboard. Exactly one visible element, the product tile "Seasonal Special", SHALL be deliberately not exposed to accessibility, as the documented example of an untestable control. Clicking it still adds the product to the order.

#### Scenario: Id survives a language switch
- **GIVEN** the demo was started with `--lang de`
- **WHEN** the element with `@Id="pay-button"` is queried
- **THEN** it resolves, and its `@Name` is the German caption instead of `Pay` (real provider only, on both platforms)

#### Scenario: The unexposed tile is absent from the tree
- **GIVEN** the Register tab is shown with category Food selected
- **WHEN** the tree under the main window is searched for `@Name="Seasonal Special"`
- **THEN** nothing is found, while the tile is visible on screen and a click at its position adds "Seasonal Special" to the order

### Requirement: Localization
With `--lang de` every visible text SHALL be German: window titles, captions, product and category names, status texts, and number formatting (decimal comma, `9,20`). Ids, seed data values and the behaviour SHALL be unchanged.

#### Scenario: German number formatting
- **GIVEN** the demo was started with `--lang de`
- **WHEN** order #1017 is read from the Orders table
- **THEN** its Total cell reads `9,20`

### Requirement: Register
The main window SHALL be the register. It SHALL contain the following parts:

- **Menu bar:**
  - *Cafe*: Settings, Exit.
  - *Orders*: New Order Ctrl+N, Kitchen Display Ctrl+K. New Order clears the current order; if it has lines, it first asks "Discard order #<n>?". An order number is only used up by a payment.
  - *Reports*: Daily Report.
  - *Help*: About.

  Its menus SHALL be popups inside the window, so that their items reach the accessibility tree on every platform.
- **Tab bar:** Register, Orders and Stock.
- **Category tree:** Coffee (Espresso-based, Filter), Tea, and Food (Sweet, Savory). It filters the product tiles.
- **Product tiles:**
  - Each tile shows the product name and price.
  - Clicking a drink opens the customization dialog. Clicking a food item adds one to the order directly.
- **Order panel:**
  - A header `Order #<n>` and one line per order item.
  - Each line has a context menu: Edit, Duplicate, Remove, and the submenu *Discount* with 10 %, 20 % and Staff (50 %).
  - Below the lines: Subtotal, Discount and Total, then a Pay button that is enabled only while the order has at least one line.
- **Status bar:** the number of orders in the kitchen, and a last-action text that reports every completed action.

The customization dialog SHALL sit inside the main window and be titled with the product name. It SHALL contain:

- the size (S, M, L; default M) as radio buttons;
- the milk (Whole, Oat, Soy, None) as a combo box, for drinks with milk;
- Extra shot and Syrup as checkboxes;
- a sweetness slider from 0 to 3;
- an optional "Name on cup" text field of at most 20 characters;
- a live price preview;
- the buttons Add to order and Cancel.

#### Scenario: Customized drink and food add up to the documented total
- **GIVEN** a new order in the register
- **WHEN** Cappuccino is added with size L, oat milk and an extra shot, and Croissant is added twice
- **THEN** the order shows two lines, the element `@Id="order-total"` reads `9.20`, and the last-action text reads `Last action: Added Croissant to order #1041`

#### Scenario: Pay is disabled for an empty order
- **GIVEN** a new order without lines
- **WHEN** the Pay button's `@IsEnabled` is read
- **THEN** it is `false`; after one item is added it becomes `true`

#### Scenario: Discount through a submenu
- **GIVEN** an order whose only line is one Latte M (3.60)
- **WHEN** the line's context menu *Discount ▸ 20 %* is chosen
- **THEN** Discount reads `0.72` and Total reads `2.88`

#### Scenario: Keyboard shortcut clears the order after confirmation
- **GIVEN** the current order #1041 has one line
- **WHEN** Ctrl+N is pressed in the main window and "Discard order #1041?" is answered with Yes
- **THEN** the order header still reads `Order #1041` and the order has no lines

### Requirement: Payment
Pay SHALL open a payment dialog inside the main window, offering Cash and Card.

- **Cash:**
  - Its confirm button SHALL be enabled only while "Amount given" is at least the total.
  - It SHALL show the change before confirming.
- **Card:**
  - It SHALL ask for a PIN in a masked field that never shows the digits.
  - After confirming, it SHALL show "Waiting for terminal…" with a progress indicator for the terminal delay, then complete.
  - The PIN `0000` SHALL always be declined. The dialog then SHALL stay open with the message "Card declined", and the order SHALL stay unpaid.
- **A completed payment SHALL:**
  - close the dialog;
  - record the order in the Orders table with status Paid;
  - send it to the kitchen;
  - report "Order #<n> paid by cash" or "paid by card";
  - start the next order.

#### Scenario: Cash payment shows the change
- **GIVEN** an order with total 9.20 and the payment dialog on Cash
- **WHEN** `10.00` is typed into "Amount given"
- **THEN** the change reads `0.80` and the confirm button is enabled; with `9.00` it is disabled

#### Scenario: Card payment waits for the terminal
- **GIVEN** an order with total 9.20 and the payment dialog on Card
- **WHEN** the PIN `1234` is entered and confirmed
- **THEN** "Waiting for terminal…" is shown, and within the terminal delay plus the default wait timeout the dialog is gone and the last-action text reads `Last action: Order #1041 paid by card`

#### Scenario: Declined card keeps the order open
- **GIVEN** an order and the payment dialog on Card
- **WHEN** the PIN `0000` is entered and confirmed
- **THEN** after the terminal delay the dialog shows "Card declined", stays open, and the order is not in the Orders table

### Requirement: Kitchen Display
Orders ▸ Kitchen Display SHALL open a separate top-level window titled `Kitchen Display`. Every paid order SHALL appear there as a ticket that exposes a group named `Order #<n>`. Each ticket SHALL list its items and a status text, going from `Brewing…` to `Ready` after the brewing delay. Each ticket SHALL have a button named `Serve`, enabled only when the ticket is Ready. Serving SHALL remove the ticket, set the order's status to Served in the Orders table, and report "Order #<n> served". Tickets SHALL be brewed one after another, in the order they were paid.

#### Scenario: Serve the right ticket among several
- **GIVEN** orders #1041 and #1042 were paid and the Kitchen Display is open
- **WHEN** the test waits until the ticket `Order #1041` shows `Ready` and clicks the `Serve` button inside that ticket
- **THEN** the ticket `Order #1041` is gone, the ticket `Order #1042` remains, and order #1041's status in the Orders table is `Served`

#### Scenario: Serve is disabled while brewing
- **GIVEN** a ticket that shows `Brewing…`
- **WHEN** its `Serve` button's `@IsEnabled` is read
- **THEN** it is `false`

### Requirement: Orders table structure
The Orders tab SHALL show the order history as a table with the columns No., Time, Barista, Items, Total, Payment and Status, one row per order, oldest first. The table element (`@Id="orders-table"`) SHALL have exactly one child element per order row, and nothing else. Each row element SHALL carry `@Id="order-<n>"`, and its children SHALL be its cells in column order. Each cell's accessible name SHALL be its displayed text. The column headers SHALL be exposed as elements named by their captions, outside the row elements. Where the platform bridge reports table roles, rows and cells SHALL use them: on AT-SPI, `item:TableRow` and `item:TableCell`.

#### Scenario: Row found by two cell values, role-independent
- **GIVEN** the Orders tab is shown
- **WHEN** `//*[@Id="orders-table"]/*[*[3][@Name="Mia"] and *[5][@Name="9.20"]]` is queried
- **THEN** exactly one element is returned, and its `@Id` is `order-1017` (real provider only, on both platforms)

#### Scenario: Rows and cells carry table roles on AT-SPI
- **GIVEN** the Orders tab is shown on Linux
- **WHEN** `count(//item:TableRow[item:TableCell[3][@Name="Mia"] and item:TableCell[5][@Name="9.20"]])` is evaluated
- **THEN** the result is `1` (real provider only, AT-SPI)

#### Scenario: The table holds only order rows
- **GIVEN** the Orders tab with the seed data only
- **WHEN** `count(//*[@Id="orders-table"]/*)` is evaluated
- **THEN** the result is `40`

### Requirement: Selecting and refunding orders
Clicking a row in the Orders table SHALL select it. Ctrl+click SHALL add a row to or remove it from the selection, and Shift+click SHALL extend it. An "Export" button SHALL be enabled only while at least one row is selected. Its caption SHALL state the number of selected rows (`Export 3 orders`, or `Export 1 order`). The last-action text SHALL report the selection count.

A row's context menu SHALL offer Refund, Reprint receipt and Copy order number. Refund SHALL be disabled for an order that is already Refunded. While "Confirm refunds" is on (the default), Refund SHALL ask "Refund order #<n>?" with Yes and No. Yes SHALL set the status to Refunded and report "Order #<n> refunded". No SHALL change nothing.

#### Scenario: Multi-select by column values
- **GIVEN** the Orders tab is shown and no row is selected
- **WHEN** every row whose Barista cell is `Jonas` and whose Payment cell is `Cash` is Ctrl+clicked
- **THEN** the Export button is enabled and its caption states exactly the number of those rows

#### Scenario: Refund with confirmation
- **GIVEN** order #1017 has status Served
- **WHEN** Refund is chosen from its context menu and the confirmation is answered with Yes
- **THEN** its Status cell reads `Refunded` and the last-action text reads `Last action: Order #1017 refunded`

#### Scenario: Refund declined changes nothing
- **GIVEN** order #1017 has status Served
- **WHEN** Refund is chosen and the confirmation is answered with No
- **THEN** its Status cell still reads `Served`

### Requirement: Stock table
The Stock tab SHALL show a table (`@Id="stock-table"`) with the columns Item, Unit, On hand, Minimum, Supplier and Reorder. It SHALL follow the same row and cell structure as the Orders table, with row ids `stock-<item>` (for example `stock-oat-milk`). The On hand cells SHALL be editable in three ways:

- Double-click opens an editor, and Enter commits the value.
- Escape cancels the edit and keeps the old value.
- A value that is not a whole number ≥ 0 SHALL be rejected. The old value then stays, and the demo reports "Invalid quantity".

Each row SHALL contain one checkbox named `Reorder`. A "Place reorder" button SHALL be enabled only while at least one Reorder checkbox is checked. Placing the reorder SHALL report "Reorder placed: <items>", listing the checked items in table order, and SHALL clear every Reorder checkbox.

#### Scenario: Edit a cell
- **GIVEN** the Stock tab is shown
- **WHEN** the On hand cell of the row `stock-oat-milk` is double-clicked, `12` is typed and Enter is pressed
- **THEN** that cell reads `12`, and `//*[@Id="stock-table"]/*[number(*[3]/@Name) < number(*[4]/@Name)]` no longer returns that row

#### Scenario: Invalid quantity is rejected
- **GIVEN** the Stock tab is shown
- **WHEN** `ten` is entered into the On hand cell of `stock-croissant`
- **THEN** the cell still reads `4` and the last-action text reads `Last action: Invalid quantity`

#### Scenario: Check the Reorder box in the rows below minimum
- **GIVEN** the Stock tab is shown with no Reorder box checked
- **WHEN** the `Reorder` checkbox is clicked in every row whose On hand is below its Minimum, and Place reorder is clicked
- **THEN** the last-action text reads `Last action: Reorder placed: Chai syrup, Croissant, Oat milk` and the Place reorder button is disabled again

### Requirement: Daily Report
Reports ▸ Daily Report SHALL open a separate top-level window titled `Daily Report`. It SHALL show `Loading…` for the report delay. Then it SHALL show a table (`@Id="report-table"`) with the columns Product, Quantity and Revenue, one row per product sold in the seed history, following the Orders table's row and cell structure. Clicking a column header SHALL sort by that column; a second click SHALL reverse the order.

#### Scenario: Report appears after loading
- **GIVEN** the Daily Report was just opened
- **WHEN** the test waits until `Loading…` is gone
- **THEN** `count(//*[@Id="report-table"]/*)` is greater than 0

### Requirement: Settings
Cafe ▸ Settings SHALL open a separate top-level window titled `Settings`. It SHALL contain the checkboxes Dark mode, Print receipts and Confirm refunds (on by default), and a combo box Default barista. Changes SHALL take effect immediately and last only until the demo exits.

#### Scenario: Refund without confirmation
- **GIVEN** Confirm refunds was switched off in Settings
- **WHEN** Refund is chosen for order #1020
- **THEN** no confirmation appears and its Status cell reads `Refunded`

### Requirement: Optional start screens
With `--splash` the demo SHALL first show a window titled `Please wait`, with the text "Heating up…", for the splash delay. It SHALL then close it and continue.

With `--login` the demo SHALL first show a window titled `Sign in`. It SHALL contain a Barista combo box, a masked PIN display and a custom-drawn keypad whose keys (`0` to `9`, `Clear`, `OK`) are exposed as buttons named by their label. The PIN `1234` SHALL sign in and open the register with that barista. Any other PIN SHALL show "Wrong PIN", clear the display and keep the window open. The PIN digits SHALL never appear in any accessible name or text.

#### Scenario: Wait for the splash to go
- **GIVEN** the demo was started with `--splash`
- **WHEN** the test waits until the window `Please wait` is gone
- **THEN** the main window `PlatynUI Cafe - Register 1` exists

#### Scenario: Wrong PIN keeps the sign-in window
- **GIVEN** the demo was started with `--login`
- **WHEN** the keys `9`, `9`, `9`, `9` and `OK` are clicked
- **THEN** "Wrong PIN" is shown, the window `Sign in` stays open, and no element on the tree carries the text `9999`

### Requirement: Closing the demo
Closing the main window SHALL close every other window of the demo and end the process. If the current order has at least one line, closing SHALL first ask "Discard order #<n>?" with Yes and No. No SHALL keep the demo running.

#### Scenario: Close with an open order asks first
- **GIVEN** the current order has one line
- **WHEN** the main window is closed
- **THEN** "Discard order #1041?" appears; after Yes the application node of the demo is gone

### Requirement: The documented launch recipe
The documentation SHALL be able to start the demo with `Start Process    platynui-demo`. It then reads the demo's process id from the application node that owns its main window:

```
data(/app:Application/*[@Name="PlatynUI Cafe - Register 1"]/parent::app:Application/@ProcessId)
```

It pins that process with `Set Root    /app:Application[@ProcessId=${pid}]`. This SHALL work on Windows even though the process `Start Process` reports is a launcher, not the demo itself.

#### Scenario: Recipe finds the demo behind a launcher
- **GIVEN** `platynui-demo` is installed as a tool, so that on Windows `Start Process` starts a launcher process
- **WHEN** `Start Process    platynui-demo` runs and `Wait Until Query` evaluates the recipe expression
- **THEN** the returned value is a number, and `/app:Application[@ProcessId=<that number>]` resolves and contains the main window (real provider only, on both platforms)

#### Scenario: Two registers are told apart by title
- **GIVEN** the demo runs twice, with `--register 1` and with `--register 2`
- **WHEN** the recipe is evaluated once with each title
- **THEN** it returns two different process ids, each owning the window with that title
