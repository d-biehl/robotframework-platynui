use super::*;

// Helper function for successful parsing assertions
fn assert_parsing_succeeds(xpath: &str, description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}' ({}): {:?}",
        xpath,
        description,
        result.err()
    );
}

// Web Automation Tests

#[rstest]
#[case("//button[@class='submit']", "Button with submit class")]
#[case(
    "//input[@type='text' and @name='username']",
    "Text input with username name"
)]
#[case("//a[contains(@href, 'example.com')]", "Link containing example.com")]
#[case("//div[@id='content']//p[1]", "First paragraph inside content div")]
#[case(
    "//form[@action='/login']//input[@type='password']",
    "Password input in login form"
)]
#[case("//table//tr[position() > 1]//td[2]", "Second cell of non-header rows")]
#[case("//span[contains(@class, 'error')]", "Span with error class")]
#[case(
    "//select[@name='country']/option[@selected]",
    "Selected option in country select"
)]
#[case("//iframe[@name='content']", "Iframe with content name")]
#[case(
    "//img[@alt and string-length(@alt) > 0]",
    "Images with non-empty alt text"
)]
fn test_web_automation_patterns(#[case] xpath: &str, #[case] description: &str) {
    assert_parsing_succeeds(xpath, description);
}

#[rstest]
#[case(
    "//button[text()='Submit' or @value='Submit']",
    "Submit button by text or value"
)]
#[case("//input[@data-testid='email-input']", "Input by test id")]
#[case(
    "//div[contains(@class, 'modal') and @style]",
    "Modal div with inline styles"
)]
#[case("//a[@href and not(starts-with(@href, '#'))]", "External links only")]
#[case("//tr[td[contains(text(), 'Total')]]", "Table rows containing Total")]
#[case(
    "//form//label[text()='Password:']/following-sibling::input",
    "Password input by label"
)]
#[case("//nav//a[contains(@class, 'active')]", "Active navigation link")]
#[case("//div[@role='alert' or @role='status']", "ARIA alert or status")]
#[case(
    "//textarea[@required and not(@disabled)]",
    "Required enabled textarea"
)]
#[case("//option[position() = last()]", "Last option in select")]
fn test_web_automation_scenarios(#[case] xpath: &str, #[case] description: &str) {
    assert_parsing_succeeds(xpath, description);
}
