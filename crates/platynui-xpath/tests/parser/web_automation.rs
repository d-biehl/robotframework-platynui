use super::*;

// Helper function for successful parsing assertions
fn assert_parsing_succeeds(xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

// Web Automation Tests

#[rstest]
#[case::button_with_submit_class("//button[@class='submit']")]
#[case::text_input_username(
    "//input[@type='text' and @name='username']",
)]
#[case::link_contains_example("//a[contains(@href, 'example.com')]")]
#[case::content_first_paragraph("//div[@id='content']//p[1]")]
#[case::password_input_in_login_form(
    "//form[@action='/login']//input[@type='password']",
)]
#[case::second_cell_non_header("//table//tr[position() > 1]//td[2]")]
#[case::span_with_error("//span[contains(@class, 'error')]")]
#[case::selected_country_option(
    "//select[@name='country']/option[@selected]",
)]
#[case::iframe_by_name("//iframe[@name='content']")]
#[case::images_non_empty_alt(
    "//img[@alt and string-length(@alt) > 0]",
)]
fn test_web_automation_patterns(#[case] xpath: &str) {
    assert_parsing_succeeds(xpath);
}

#[rstest]
#[case::submit_button_text_or_value(
    "//button[text()='Submit' or @value='Submit']",
)]
#[case::input_by_testid("//input[@data-testid='email-input']")]
#[case::modal_div_with_style(
    "//div[contains(@class, 'modal') and @style]",
)]
#[case::external_links_only("//a[@href and not(starts-with(@href, '#'))]")]
#[case::rows_containing_total("//tr[td[contains(text(), 'Total')]]")]
#[case::password_input_by_label(
    "//form//label[text()='Password:']/following-sibling::input",
)]
#[case::active_nav_link("//nav//a[contains(@class, 'active')]")]
#[case::aria_alert_or_status("//div[@role='alert' or @role='status']")]
#[case::required_enabled_textarea(
    "//textarea[@required and not(@disabled)]",
)]
#[case::last_option("//option[position() = last()]")]
fn test_web_automation_scenarios(#[case] xpath: &str) {
    assert_parsing_succeeds(xpath);
}
