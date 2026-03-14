import Foundation

enum MobileBridgePlugin {
    static let channel = "mobile_bridge"
    static let permissionsEvent = "mobile_permissions_changed"
    static let backgroundSyncEvent = "mobile_background_sync_changed"
    static let transferActionEvent = "mobile_transfer_action_requested"
}
