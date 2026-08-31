import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.layershell 1.0 as LayerShell
import org.kde.taskmanager as TaskManager

Window {
    id: root

    readonly property string controlUrl: Qt.application.arguments.length > 2
        ? Qt.application.arguments[Qt.application.arguments.length - 2] : ""
    readonly property string profileName: Qt.application.arguments.length > 1
        ? Qt.application.arguments[Qt.application.arguments.length - 1] : "Remote desktop"
    readonly property int collapsedHeight: 7
    readonly property int expandedHeight: 52
    readonly property int collapsedWidth: 76
    readonly property int expandedWidth: 420

    property int remotePid: 0
    property bool remoteFound: false
    property bool remoteWasFound: false
    property bool remoteActive: false
    property bool remoteMinimized: false
    property bool minimizeRequested: false
    property bool minimizeObserved: false
    property bool pinned: false
    property bool lingerExpanded: true
    readonly property bool expanded: pinned || lingerExpanded || panelHover.hovered
    readonly property bool sessionVisible: !remoteWasFound
        || (remoteFound && remoteActive && !remoteMinimized && !minimizeRequested)

    width: expanded ? expandedWidth : collapsedWidth
    height: expanded ? expandedHeight : collapsedHeight
    visible: true
    color: "transparent"
    flags: Qt.FramelessWindowHint
    title: profileName + " — RustRDP controls"

    Behavior on width { NumberAnimation { duration: 130; easing.type: Easing.OutCubic } }
    Behavior on height { NumberAnimation { duration: 130; easing.type: Easing.OutCubic } }

    LayerShell.Window.anchors: LayerShell.Window.AnchorTop
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.exclusionZone: 0
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    LayerShell.Window.scope: "rustrdp-session-controller"

    FontLoader {
        id: material
        source: Qt.resolvedUrl("MaterialSymbolsFilled.ttf")
    }

    TaskManager.TasksModel {
        id: tasks
        groupMode: TaskManager.TasksModel.GroupDisabled
        separateLaunchers: true
    }

    function sendCommand(command) {
        if (controlUrl.length === 0)
            return
        const request = new XMLHttpRequest()
        request.open("POST", controlUrl + "/" + command)
        request.send("")
    }

    function refreshControllerState() {
        if (controlUrl.length === 0)
            return
        const request = new XMLHttpRequest()
        request.onreadystatechange = function() {
            if (request.readyState !== XMLHttpRequest.DONE || request.status !== 200)
                return
            try {
                root.remotePid = Number(JSON.parse(request.responseText).pid || 0)
            } catch (error) {
                root.remotePid = 0
            }
        }
        request.open("GET", controlUrl + "/state")
        request.send("")
    }

    function refreshRemoteWindow() {
        let found = false
        let active = false
        let minimized = false
        if (remotePid > 0) {
            for (let row = 0; row < tasks.count; ++row) {
                const index = tasks.index(row, 0)
                if (Number(tasks.data(index, TaskManager.AbstractTasksModel.AppPid)) !== remotePid)
                    continue
                found = true
                active = Boolean(tasks.data(index, TaskManager.AbstractTasksModel.IsActive))
                minimized = Boolean(tasks.data(index, TaskManager.AbstractTasksModel.IsMinimized))
                break
            }
        }
        remoteFound = found
        remoteWasFound = remoteWasFound || found
        remoteActive = active
        remoteMinimized = minimized
        if (minimizeRequested && minimized)
            minimizeObserved = true
        if (minimizeObserved && active && !minimized) {
            minimizeRequested = false
            minimizeObserved = false
        }
        if (sessionVisible) {
            if (!root.visible)
                root.show()
        } else if (root.visible) {
            root.hide()
        }
    }

    Timer {
        interval: 200
        running: true
        repeat: true
        onTriggered: {
            root.refreshControllerState()
            root.refreshRemoteWindow()
        }
    }

    Timer {
        interval: 1700
        running: true
        repeat: false
        onTriggered: root.lingerExpanded = false
    }

    Timer {
        id: collapseTimer
        interval: 550
        repeat: false
        onTriggered: root.lingerExpanded = false
    }

    Timer {
        interval: 150
        running: true
        repeat: false
        onTriggered: root.sendCommand("ready")
    }

    HoverHandler {
        id: panelHover
        onHoveredChanged: {
            if (hovered) {
                collapseTimer.stop()
                root.lingerExpanded = true
            } else if (!root.pinned) {
                collapseTimer.restart()
            }
        }
    }

    component PanelButton: Rectangle {
        id: button
        required property string glyph
        required property string accessibleName
        property color foreground: "#dce3ea"
        property color hoverColor: "#39444f"
        signal triggered()

        width: 34
        height: 34
        radius: 7
        color: pointer.containsMouse ? hoverColor : "transparent"
        Accessible.name: accessibleName
        Accessible.role: Accessible.Button

        Text {
            anchors.centerIn: parent
            text: button.glyph
            color: button.foreground
            font.family: material.name
            font.pixelSize: 19
        }

        MouseArea {
            id: pointer
            anchors.fill: parent
            hoverEnabled: true
            onClicked: button.triggered()
        }

        ToolTip.visible: pointer.containsMouse
        ToolTip.text: accessibleName
        ToolTip.delay: 500
    }

    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        width: root.collapsedWidth
        height: root.collapsedHeight
        radius: 4
        color: "#4ea1f2"
        opacity: root.expanded ? 0 : 0.96
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 3
        radius: 10
        color: "#f220242b"
        border.width: 1
        border.color: "#4a5663"
        opacity: root.expanded ? 1 : 0

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 6
            spacing: 7

            Text {
                text: "\ue30c"
                color: "#4ea1f2"
                font.family: material.name
                font.pixelSize: 21
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0

                Text {
                    Layout.fillWidth: true
                    text: root.profileName
                    color: "#f2f4f7"
                    font.pixelSize: 13
                    font.bold: true
                    elide: Text.ElideRight
                }

                Text {
                    text: "Remote session"
                    color: "#9aa3ae"
                    font.pixelSize: 10
                }
            }

            PanelButton {
                glyph: "\uf10d"
                accessibleName: root.pinned ? "Auto-hide controls" : "Keep controls open"
                foreground: root.pinned ? "#4ea1f2" : "#dce3ea"
                onTriggered: {
                    root.pinned = !root.pinned
                    root.lingerExpanded = root.pinned
                }
            }

            PanelButton {
                glyph: "\ue30c"
                accessibleName: "Open RustRDP"
                onTriggered: root.sendCommand("open-app")
            }

            PanelButton {
                glyph: root.minimizeRequested || root.remoteMinimized ? "\ue6fa" : "\ue15b"
                accessibleName: root.minimizeRequested || root.remoteMinimized
                    ? "Show remote desktop" : "Minimize remote desktop"
                onTriggered: {
                    if (root.minimizeRequested || root.remoteMinimized) {
                        root.minimizeRequested = false
                        root.minimizeObserved = false
                        root.sendCommand("restore")
                    } else {
                        root.minimizeRequested = true
                        root.sendCommand("minimize")
                        root.hide()
                    }
                }
            }

            Rectangle {
                width: 1
                height: 24
                color: "#4a5663"
            }

            PanelButton {
                glyph: "\ue047"
                accessibleName: "Disconnect"
                foreground: "#f08a93"
                hoverColor: "#59343a"
                onTriggered: root.sendCommand("disconnect")
            }
        }
    }
}
