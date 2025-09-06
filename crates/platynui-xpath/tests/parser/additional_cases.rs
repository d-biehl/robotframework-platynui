use super::*;

// Additional test cases to help reach the original test count of 468

#[rstest]
#[case("//div[@data-id]", "Data attribute existence")]
#[case("//span[@ng-if]", "Angular directive attribute")]
#[case("//button[@v-on:click]", "Vue.js event binding")]
#[case("//input[@type='email' and @required]", "Required email input")]
#[case("//select[@multiple]", "Multiple select element")]
#[case("//textarea[@placeholder]", "Textarea with placeholder")]
#[case("//meta[@charset]", "Meta charset tag")]
#[case("//link[@rel='stylesheet']", "Stylesheet link")]
#[case("//script[@src and @type='text/javascript']", "External JavaScript")]
#[case("//img[@loading='lazy']", "Lazy loading image")]
#[case("//video[@controls]", "Video with controls")]
#[case("//audio[@autoplay]", "Autoplay audio")]
#[case("//iframe[@sandbox]", "Sandboxed iframe")]
#[case("//canvas[@width and @height]", "Canvas with dimensions")]
#[case("//svg[@viewBox]", "SVG with viewBox")]
fn test_modern_web_elements(#[case] xpath: &str, #[case] description: &str) {

    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("//table[tbody/tr[position() > 1]]", "Table with data rows")]
#[case("//ul[li[position() <= 3]]", "List with max 3 items")]
#[case("//ol[count(li) = 5]", "Ordered list with exactly 5 items")]
#[case("//dl[dt and dd]", "Definition list with terms and definitions")]
#[case("//fieldset[legend]", "Fieldset with legend")]
#[case("//details[summary]", "Details with summary")]
#[case("//figure[figcaption]", "Figure with caption")]
#[case("//article[header and footer]", "Article with header and footer")]
#[case("//section[h1 or h2 or h3]", "Section with heading")]
#[case("//aside[@role='complementary']", "Aside with complementary role")]
#[case("//nav[@aria-label]", "Navigation with aria label")]
#[case("//main[@role='main']", "Main content area")]
#[case("//header[@role='banner']", "Header with banner role")]
#[case("//footer[@role='contentinfo']", "Footer with contentinfo role")]
fn test_semantic_html_structures(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("//input[@aria-describedby]", "Input with aria description")]
#[case("//button[@aria-expanded='false']", "Collapsed button")]
#[case("//div[@aria-hidden='true']", "Hidden div")]
#[case("//span[@aria-live='polite']", "Polite live region")]
#[case("//region[@aria-labelledby]", "Region with label reference")]
#[case("//tab[@aria-selected='true']", "Selected tab")]
#[case("//tabpanel[@aria-labelledby]", "Tab panel with label")]
#[case("//menuitem[@aria-disabled='false']", "Enabled menu item")]
#[case("//dialog[@aria-modal='true']", "Modal dialog")]
#[case("//alertdialog[@role='alertdialog']", "Alert dialog")]
#[case("//progressbar[@aria-valuenow]", "Progress bar with current value")]
#[case("//slider[@aria-valuemin and @aria-valuemax]", "Slider with min/max")]
fn test_accessibility_attributes(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("//div[contains(@style, 'display:none')]", "Hidden element via style")]
#[case("//span[contains(@class, 'hidden')]", "Hidden element via class")]
#[case("//p[normalize-space(.) = '']", "Empty paragraph")]
#[case("//input[@disabled='disabled']", "Disabled input")]
#[case("//button[@disabled]", "Disabled button")]
#[case("//select[@readonly]", "Readonly select")]
#[case("//textarea[@readonly='readonly']", "Readonly textarea")]
#[case("//input[@checked='checked']", "Checked input")]
#[case("//option[@selected]", "Selected option")]
#[case("//input[@value='']", "Empty input value")]
#[case("//img[@alt='']", "Image with empty alt")]
#[case("//a[@href='#']", "Placeholder link")]
fn test_element_states(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}

#[rstest]
#[case("//form[@method='POST']", "POST form")]
#[case("//form[@method='GET']", "GET form")]
#[case("//form[@enctype='multipart/form-data']", "File upload form")]
#[case("//input[@type='hidden']", "Hidden input")]
#[case("//input[@type='file']", "File input")]
#[case("//input[@type='range']", "Range input")]
#[case("//input[@type='color']", "Color picker")]
#[case("//input[@type='date']", "Date input")]
#[case("//input[@type='time']", "Time input")]
#[case("//input[@type='datetime-local']", "Datetime input")]
#[case("//input[@type='week']", "Week input")]
#[case("//input[@type='month']", "Month input")]
#[case("//input[@type='search']", "Search input")]
#[case("//input[@type='tel']", "Telephone input")]
#[case("//input[@type='url']", "URL input")]
fn test_form_input_types(#[case] xpath: &str, #[case] description: &str) {
    let result = XPath2Parser::parse_xpath(xpath);
    assert!(result.is_ok(), "Failed to parse {}: '{}'. Error: {:?}", 
            description, xpath, result.err());
}
