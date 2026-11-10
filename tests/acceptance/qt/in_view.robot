*** Settings ***
Documentation       Real acceptance for ``IsInView`` on AT-SPI (spec ``in-view-attribute``): an element is
...                 in view only while it lies fully inside the area its ancestors show. Runs against an
...                 instance launched with ``--in-view``: a tab bar too narrow for its tabs, whose later
...                 tabs start scrolled out of it, and a combo box. Qt places a combo box's list and the
...                 menu bar's File menu below the widget that opens them, although both extend beyond it;
...                 their entries must count as in view while open and not while closed.
...
...                 platform:x11 — on UIA a partly visible element counts as in view, and the Wayland
...                 lane is not verified.

Resource            resources/testapp.resource

Suite Setup         Launch Qt Test App    PlatynUI Test App (Qt In View)    org.platynui.test.qt.inview    --in-view
Suite Teardown      Terminate Qt Instance

Test Tags           real    platform:x11


*** Test Cases ***
A Tab Beyond The Tab Bar Comes Into View By Scrolling
    [Documentation]    The last tab starts scrolled out of the tab bar. Clicking ``Scroll Right``
    ...    until it reads as in view must end, and then the first tab is scrolled out instead.
    BM.Get Attribute Value    .//item:*[@Name="in-view-tab-12"]    IsInView    ==    ${False}
    WHILE    True    limit=15
        ${in_view}=    BM.Get Attribute Value    .//item:*[@Name="in-view-tab-12"]    IsInView
        IF    ${in_view}    BREAK
        BM.Pointer Click    .//*[@Name="Scroll Right"]
    END
    BM.Wait Until Attribute Value    .//item:*[@Name="in-view-tab-01"]    IsInView    ==    ${False}

The Entries Of An Open Combo Box Are In View
    [Documentation]    The entries are not in view while the combo box is closed, in view once it is
    ...    open, and out of view again after one of them is chosen, which closes the list. Qt names a
    ...    combo box after its current entry, so the suite's only combo box is located by its role.
    BM.Get Attribute Value    .//ComboBox//item:*[@Name="in-view-entry-2"]    IsInView    ==    ${False}
    BM.Pointer Click    .//ComboBox
    BM.Wait Until Attribute Value    .//ComboBox//item:*[@Name="in-view-entry-2"]    IsInView    ==    ${True}
    BM.Pointer Click    .//ComboBox//item:*[@Name="in-view-entry-2"]
    BM.Wait Until Attribute Value    .//ComboBox//item:*[@Name="in-view-entry-2"]    IsInView    ==    ${False}

The Entries Of An Open Menu Are In View
    [Documentation]    The entries of the File menu are not in view while it is closed, in view once
    ...    it is open, and out of view again after one of them is activated, which closes the menu.
    BM.Get Attribute Value    .//MenuItem[@Name="menu-file-new"]    IsInView    ==    ${False}
    BM.Pointer Click    .//MenuItem[@Name="File"]
    BM.Wait Until Attribute Value    .//MenuItem[@Name="menu-file-new"]    IsInView    ==    ${True}
    BM.Pointer Click    .//MenuItem[@Name="menu-file-new"]
    BM.Wait Until Exists    .//*[@Name="last-action-menu-file-new"]
    BM.Wait Until Attribute Value    .//MenuItem[@Name="menu-file-new"]    IsInView    ==    ${False}
