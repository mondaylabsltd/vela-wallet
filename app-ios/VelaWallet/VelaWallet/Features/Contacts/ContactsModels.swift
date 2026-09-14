//
//  ContactsModels.swift
//  VelaWallet
//
//  Contacts view models (spec 018, data-model.md). Components consume ONLY
//  these display-ready shapes — no service types, no sorting, no
//  collation, no formatting, no fetching (FR-005 / SC-002 / SC-005). The
//  later "real contacts data" feature replaces the fixture layer that
//  builds them and nothing else. Mirrors the web reference
//  `src/lib/contacts/model.ts`.
//

import SwiftUI

/// The nine mobile contacts states (data-model.md §Screen states).
enum ContactsStateId: String, CaseIterable, Identifiable {
    case c1, c1s, c1f, c2, c2s, c3, c4, c5, c6, c7, c8, c9
    var id: String { rawValue }

    /// Gallery chip label — mock/state code, not translatable copy.
    var label: String {
        switch self {
        case .c1: "C1"
        case .c1s: "C1s"
        case .c1f: "C1f"
        case .c2: "C2"
        case .c2s: "C2s"
        case .c3: "C3"
        case .c4: "C4"
        case .c5: "C5"
        case .c6: "C6"
        case .c7: "C7"
        case .c8: "C8"
        case .c9: "C9"
        }
    }
}

// MARK: - Contacts & groups

struct ContactModel: Identifiable {
    let id = UUID()
    let name: String
    /// Middle-truncated, pre-computed by the fixture layer.
    let addressDisplay: String
    /// Raw seed — IdenticonAvatar normalizes through vela-core (FR-006);
    /// never lowercased at the call site (spec 003 rule).
    let addressFull: String
    let sectionKey: String
    let groups: [String]
}

struct ContactSectionModel: Identifiable {
    let id = UUID()
    /// Uppercase letter header (A, B, C …).
    let letter: String
    let contacts: [ContactModel]
}

struct GroupRowModel: Identifiable {
    let id = UUID()
    let name: String
    /// Pre-resolved `3 人`.
    let countLabel: String
}

// MARK: - Search

struct ContactsSearchModel {
    /// 搜索名字、ENS 或地址.
    let placeholder: String
    /// Non-nil in the search-active state (C1f).
    var query: String?
    /// Resolved a11y label for the clear affordance.
    let clearLabel: String
}

// MARK: - Menus (sheet / confirm)

struct MenuItemModel: Identifiable {
    let id = UUID()
    let icon: LucideGlyph
    let label: String
    var destructive: Bool = false
    /// Hairline above this row (data-model `dividersAfter`).
    var dividerAbove: Bool = false
}

/// Mobile ActionMenuSheet content (C5, C6 and the delete-confirm variant).
struct ActionMenuModel {
    /// Delete-confirm variant only (删除联系人？).
    var title: String?
    /// Delete-confirm body ({{name}} 将从通讯录中移除。).
    var body: String?
    let items: [MenuItemModel]
    /// Separate 取消 button under the rows.
    let cancel: String
}

// MARK: - Swipe reveal (C1s)

struct SwipeRevealModel {
    /// Index into the flattened contact list (sections in order).
    let contactId: UUID
    /// 转账 (accent).
    let sendLabel: String
    /// 删除 (destructive).
    let deleteLabel: String
}

// MARK: - Empty state (C3)

struct EmptyCTAModel {
    let title: String
    let caption: String
    /// Accent primary — 添加联系人.
    let primary: String
    /// Outline secondary — 从文件导入.
    let secondary: String
}

// MARK: - Screens

struct ContactsHomeModel {
    let state: ContactsStateId
    /// 通讯录.
    let title: String
    /// Resolved a11y label for the header add button.
    let addLabel: String
    let search: ContactsSearchModel
    /// 分组 + 管理 (nil in the empty state).
    var groupsHeader: (title: String, action: String)?
    var groups: [GroupRowModel] = []
    /// 联系人 + 8 位 (nil in the empty state).
    var contactsHeader: (title: String, action: String)?
    var sections: [ContactSectionModel] = []
    /// Full A–Z + # (research D4) — rendered regardless of which
    /// sections exist.
    var indexLetters: [String] = []
    var empty: EmptyCTAModel?
    /// Search returned nothing — reused empty treatment (spec edge case).
    var searchEmpty: EmptyCTAModel?
    let tabs: TabsModel
    var reveal: SwipeRevealModel?
    var sheet: ActionMenuModel?
    /// The add form, over this screen (C7).
    var form: ContactFormModel?
    /// Pre-resolved destructive confirm per contact (row-swipe 删除 →
    /// second confirmation, FR-008). Display-ready; keyed by contact id so
    /// the screen never touches the i18n layer.
    var deleteConfirms: [UUID: ActionMenuModel] = [:]
    let textScale: CGFloat
}

/// C7 / C8 — the add form (empty, Save gated) and the edit form (filled).
///
/// Drawn from 018's vocabulary and nothing else: two labelled fields, the
/// core's one error line, Save and Cancel. Every string here is the core's;
/// the shell decides only where they sit.
struct ContactFormModel {
    /// 新建联系人 / 编辑联系人.
    let title: String
    let nameLabel: String
    let namePlaceholder: String
    let addressLabel: String
    let addressPlaceholder: String
    let name: String
    let address: String
    /// The core's refusal, once there is something to refuse.
    var error: String?
    let save: String
    let cancel: String
    /// The CORE's gate. A form that decided this itself would eventually
    /// let through a contact the core then rejects, with nothing on screen
    /// to explain the difference.
    let saveEnabled: Bool
    /// Editing an existing contact: the address IS the contact's identity in
    /// the address book, so it stays put and a new address means a new person.
    var addressLocked = false
}

/// The star on a contact's detail. `on` is the core's `favorite`.
struct FavouriteControlModel {
    let on: Bool
    /// The accessible name — 收藏, the same word the home's section uses.
    let label: String
}

/// What the core found out about this address when the page opened
/// (`InspectRecipient`): whether it is a contract wallet, and whether this
/// person has ever been paid before.
struct ContactInspectionModel {
    var tag: String?
    var firstTime: String?
}

struct ContactActionsModel {
    /// 转账 / 收款 / 二维码 (mock order).
    let send: String
    let receive: String
    let qr: String
}

struct ContactDetailModel {
    let state: ContactsStateId
    let contact: ContactModel
    /// Group memberships (家人) — the trailing add chip is `addChip`.
    let chips: [String]
    /// 分组 (rendered as `+ 分组`).
    let addChip: String
    let actions: ContactActionsModel
    /// 地址.
    let addressLabel: String
    /// Exactly the mock's two mono lines.
    let addressLines: [String]
    let copyLabel: String
    let copiedLabel: String
    /// 最近往来 + 全部.
    let activityTitle: String
    let activityAction: String
    let activity: [ActivityRowModel]
    /// Reused empty treatment when a contact has no activity (edge case).
    var activityEmpty: SectionEmptyModel?
    /// 删除联系人.
    let deleteLabel: String
    /// Resolved a11y labels for the header controls.
    let backLabel: String
    let editLabel: String
    /// The star in the header (C9).
    var favourite: FavouriteControlModel?
    /// The core's reading of this address (C9).
    var inspection: ContactInspectionModel?
    var sheet: ActionMenuModel?
    /// The add/edit form, over this screen (C8).
    var form: ContactFormModel?
    let textScale: CGFloat
}

struct GroupDetailModel {
    let state: ContactsStateId
    /// 家人.
    let name: String
    /// 3 位成员.
    let membersLabel: String
    let members: [ContactModel]
    /// 添加成员.
    let addMemberLabel: String
    /// 群发转账.
    let ctaLabel: String
    /// 向本组 3 人转账，金额可分别设置。
    let ctaCaption: String
    /// Empty group (0 members) disables the pinned CTA (spec edge case).
    let ctaEnabled: Bool
    let backLabel: String
    let moreLabel: String
    var sheet: ActionMenuModel?
    let textScale: CGFloat
}

/// One fixture state resolves to exactly one of the three mobile screens.
enum ContactsScene {
    case home(ContactsHomeModel)
    case detail(ContactDetailModel)
    case group(GroupDetailModel)

    var state: ContactsStateId {
        switch self {
        case .home(let model): model.state
        case .detail(let model): model.state
        case .group(let model): model.state
        }
    }

    var home: ContactsHomeModel? {
        if case .home(let model) = self { return model }
        return nil
    }

    var detail: ContactDetailModel? {
        if case .detail(let model) = self { return model }
        return nil
    }

    var group: GroupDetailModel? {
        if case .group(let model) = self { return model }
        return nil
    }
}
