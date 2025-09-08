use super::*;

// Additional test cases to help reach the original test count of 468

#[rstest]
#[case::data_attr("//div[@data-id]")]
#[case::angular_attr("//span[@ng-if]")]
#[case::vue_event("//button[@v-on:click]")]
#[case::email_input_required("//input[@type='email' and @required]")]
#[case::select_multiple("//select[@multiple]")]
#[case::textarea_placeholder("//textarea[@placeholder]")]
#[case::meta_charset("//meta[@charset]")]
#[case::link_stylesheet("//link[@rel='stylesheet']")]
#[case::script_external("//script[@src and @type='text/javascript']")]
#[case::img_lazy("//img[@loading='lazy']")]
#[case::video_controls("//video[@controls]")]
#[case::audio_autoplay("//audio[@autoplay]")]
#[case::iframe_sandbox("//iframe[@sandbox]")]
#[case::canvas_dimensions("//canvas[@width and @height]")]
#[case::svg_viewbox("//svg[@viewBox]")]
fn test_modern_web_elements(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::table_data_rows("//table[tbody/tr[position() > 1]]")]
#[case::list_max_3("//ul[li[position() <= 3]]")]
#[case::ol_exactly_5("//ol[count(li) = 5]")]
#[case::dl_terms_defs("//dl[dt and dd]")]
#[case::fieldset_legend("//fieldset[legend]")]
#[case::details_summary("//details[summary]")]
#[case::figure_figcaption("//figure[figcaption]")]
#[case::article_header_footer("//article[header and footer]")]
#[case::section_with_heading("//section[h1 or h2 or h3]")]
#[case::aside_complementary("//aside[@role='complementary']")]
#[case::nav_aria_label("//nav[@aria-label]")]
#[case::main_role("//main[@role='main']")]
#[case::header_banner("//header[@role='banner']")]
#[case::footer_contentinfo("//footer[@role='contentinfo']")]
fn test_semantic_html_structures(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::aria_describedby("//input[@aria-describedby]")]
#[case::aria_expanded_false("//button[@aria-expanded='false']")]
#[case::aria_hidden_true("//div[@aria-hidden='true']")]
#[case::aria_live_polite("//span[@aria-live='polite']")]
#[case::aria_labelledby("//region[@aria-labelledby]")]
#[case::aria_selected_true("//tab[@aria-selected='true']")]
#[case::tabpanel_labelledby("//tabpanel[@aria-labelledby]")]
#[case::menuitem_enabled("//menuitem[@aria-disabled='false']")]
#[case::dialog_modal("//dialog[@aria-modal='true']")]
#[case::alertdialog_role("//alertdialog[@role='alertdialog']")]
#[case::progressbar_value_now("//progressbar[@aria-valuenow]")]
#[case::slider_min_max("//slider[@aria-valuemin and @aria-valuemax]")]
fn test_accessibility_attributes(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::hidden_via_style("//div[contains(@style, 'display:none')]")]
#[case::hidden_via_class("//span[contains(@class, 'hidden')]")]
#[case::empty_paragraph("//p[normalize-space(.) = '']")]
#[case::disabled_input("//input[@disabled='disabled']")]
#[case::disabled_button("//button[@disabled]")]
#[case::readonly_select("//select[@readonly]")]
#[case::readonly_textarea("//textarea[@readonly='readonly']")]
#[case::checked_input("//input[@checked='checked']")]
#[case::selected_option("//option[@selected]")]
#[case::empty_input_value("//input[@value='']")]
#[case::empty_alt_image("//img[@alt='']")]
#[case::placeholder_link("//a[@href='#']")]
fn test_element_states(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}

#[rstest]
#[case::form_post("//form[@method='POST']")]
#[case::form_get("//form[@method='GET']")]
#[case::form_multipart("//form[@enctype='multipart/form-data']")]
#[case::input_hidden("//input[@type='hidden']")]
#[case::input_file("//input[@type='file']")]
#[case::input_range("//input[@type='range']")]
#[case::input_color("//input[@type='color']")]
#[case::input_date("//input[@type='date']")]
#[case::input_time("//input[@type='time']")]
#[case::input_datetime_local("//input[@type='datetime-local']")]
#[case::input_week("//input[@type='week']")]
#[case::input_month("//input[@type='month']")]
#[case::input_search("//input[@type='search']")]
#[case::input_tel("//input[@type='tel']")]
#[case::input_url("//input[@type='url']")]
fn test_form_input_types(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}", xpath, result.err()
    );
}
