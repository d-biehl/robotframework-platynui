*** Settings ***
Documentation     Mock-backed checks that a selector matching several elements resolves in document
...               order, the order in which the elements appear in the tree: an element's children
...               come right after it, before its next sibling. The Operations Console holds tree items
...               on two levels, Dashboard (with Overview and Metrics) and Reports (with Täglich,
...               Monatlich and Jährlich), so the first item that is not Dashboard is Overview.
...               The mock names its tree items in ``item:Name``.
...
...               A position inside a step counts per parent, a position on a parenthesized selector
...               counts over all matches: ``.//item:TreeItem[2]`` is the second item of every parent,
...               ``(.//item:TreeItem)[2]`` is the second item overall.
Library           PlatynUI.BareMetal    use_mock=${True}    query_settings={'timeout': 0.2}

Suite Setup       Set Root    //control:Window[@Name="Operations Console"]    scope=SUITE


*** Test Cases ***
A Keyword Acts On The First Match In Document Order
    ${name}=    Get Attribute Value    .//item:TreeItem[@item:Name!="Dashboard"]    item:Name
    Should Be Equal    ${name}    Overview

Query Only First Returns The First Match In Document Order
    ${item}=    Query    .//item:TreeItem[@item:Name!="Dashboard"]    only_first=${True}
    Should Be Equal    ${item.name}    Overview

Query Lists The Matches In Document Order
    ${items}=    Query    .//item:TreeItem
    Should Be Equal    ${{ [item.name for item in $items] }}
    ...    ${{ ['Dashboard', 'Overview', 'Metrics', 'Reports', 'Täglich', 'Monatlich', 'Jährlich'] }}

A Parenthesized Selector Counts Over All Matches
    Get Attribute Value    (.//item:TreeItem)[2]    item:Name    ==    Overview

A Position In A Step Counts Per Parent
    ${items}=    Query    .//item:TreeItem[2]
    Should Be Equal    ${{ [item.name for item in $items] }}    ${{ ['Metrics', 'Reports', 'Monatlich'] }}

A Position That No Parent Reaches Matches Nothing
    Run Keyword And Expect Error
    ...    STARTS:ElementNotFoundError: No element matched './/item:TreeItem[4]' within timeout of 0.2 seconds
    ...    Get Attribute Value    .//item:TreeItem[4]    item:Name
