import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.layershell 1.0 as LayerShell

Window {
    id: root

    readonly property string controlUrl: Qt.application.arguments.length > 2
        ? Qt.application.arguments[Qt.application.arguments.length - 3] : ""
    readonly property string profileName: Qt.application.arguments.length > 1
        ? Qt.application.arguments[Qt.application.arguments.length - 2] : "Remote desktop"
    readonly property bool canArrange: Qt.application.arguments.length > 0
        && Qt.application.arguments[Qt.application.arguments.length - 1] === "arrange"
    readonly property int collapsedHeight: 7
    readonly property int expandedHeight: 52
    readonly property int collapsedWidth: 76
    readonly property int expandedWidth: canArrange ? 620 : 420

    property bool remoteFound: false
    property bool remoteWasFound: false
    property bool remoteActive: false
    property bool remoteMinimized: false
    property real remoteSizePercent: 0
    property bool minimizeRequested: false
    property bool minimizeObserved: false
    property bool pinned: false
    property Item hoveredControl: null
    property bool hintReady: false
    readonly property bool hintVisible: expanded && hoveredControl !== null && hintReady
    // Keep the native surface stable while hovering. Only the label's visibility
    // changes; its transparent space must not trigger a Layer Shell resize.
    readonly property int panelHeight: expanded ? expandedHeight + 28 : collapsedHeight
    onHoveredControlChanged: {
        hintReady = false
        hintDelay.stop()
        if (hoveredControl !== null)
            hintDelay.restart()
    }
    property bool lingerExpanded: true
    readonly property bool expanded: pinned || lingerExpanded || panelHover.hovered
    readonly property bool sessionVisible: !remoteWasFound
        || (remoteFound && remoteActive && !remoteMinimized && !minimizeRequested)

    onExpandedChanged: {
        root.width = root.expanded ? root.expandedWidth : root.collapsedWidth
    }
    onPanelHeightChanged: root.height = panelHeight

    width: expanded ? expandedWidth : collapsedWidth
    height: panelHeight
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

    Timer {
        id: hintDelay
        interval: 700
        onTriggered: root.hintReady = true
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
                const state = JSON.parse(request.responseText)
                root.refreshRemoteWindow(state)
            } catch (error) {
                console.warn("Invalid remote window state: " + error)
            }
        }
        request.open("GET", controlUrl + "/state")
        request.send("")
    }

    function refreshRemoteWindow(state) {
        const found = Boolean(state.found)
        const active = Boolean(state.active)
        const minimized = Boolean(state.minimized)
        remoteSizePercent = Number(state.sizePercent || 0)
        if (!sizeSlider.pressed && remoteSizePercent > 0)
            sizeSlider.value = remoteSizePercent
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
            onEntered: root.hoveredControl = button
            onExited: {
                if (root.hoveredControl === button)
                    root.hoveredControl = null
            }
            onClicked: button.triggered()
        }
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
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 3
        height: root.expandedHeight - 6
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
                visible: root.canArrange
                glyph: "\ue89f"
                accessibleName: "Move remote window"
                onTriggered: root.sendCommand("move")
            }

            Slider {
                id: sizeSlider
                visible: root.canArrange
                Layout.preferredWidth: 120
                from: 1
                to: 100
                stepSize: 1
                value: 1
                enabled: root.remoteSizePercent > 0
                snapMode: Slider.SnapAlways
                Accessible.name: "Remote window size"
                Accessible.description: Math.round(value) + "% of the display, preserving window proportions"
                onHoveredChanged: {
                    if (hovered)
                        root.hoveredControl = sizeSlider
                    else if (root.hoveredControl === sizeSlider)
                        root.hoveredControl = null
                }

                onPressedChanged: {
                    if (!pressed)
                        root.sendCommand("resize/" + Math.round(value))
                }

                background: Rectangle {
                    x: sizeSlider.leftPadding
                    y: sizeSlider.topPadding + sizeSlider.availableHeight / 2 - height / 2
                    width: sizeSlider.availableWidth
                    height: 4
                    radius: 2
                    color: "#4a5663"

                    Rectangle {
                        width: sizeSlider.visualPosition * parent.width
                        height: parent.height
                        radius: parent.radius
                        color: "#4ea1f2"
                    }
                }

                handle: Rectangle {
                    x: sizeSlider.leftPadding + sizeSlider.visualPosition
                       * (sizeSlider.availableWidth - width)
                    y: sizeSlider.topPadding + sizeSlider.availableHeight / 2 - height / 2
                    implicitWidth: 16
                    implicitHeight: 16
                    radius: 8
                    color: sizeSlider.pressed ? "#dceeff" : "#f2f4f7"
                    border.width: 2
                    border.color: "#4ea1f2"
                }

            }

            Label {
                visible: root.canArrange
                Layout.preferredWidth: 40
                text: root.remoteSizePercent > 0
                    ? Math.round(sizeSlider.pressed ? sizeSlider.value : root.remoteSizePercent) + "%"
                    : "—"
                color: "#dce3ea"
                font.pixelSize: 12
                horizontalAlignment: Text.AlignRight
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

    // Drawn inside this surface, never a native popup that can cover its buttons.
    Rectangle {
        id: hoverLabel
        visible: root.hintVisible
        enabled: false
        y: root.expandedHeight + 2
        width: Math.min(hintText.implicitWidth + 16, root.width - 6)
        height: 24
        readonly property real controlCenter: root.hoveredControl
            ? root.hoveredControl.mapToItem(root.contentItem, root.hoveredControl.width / 2, 0).x : 0
        x: Math.max(3, Math.min(controlCenter - width / 2, root.width - width - 3))
        radius: 5
        color: "#f220242b"
        border.color: "#4a5663"
        Text {
            id: hintText
            anchors.centerIn: parent
            width: parent.width - 16
            text: root.hoveredControl ? root.hoveredControl.Accessible.name : ""
            color: "#dce3ea"
            font.pixelSize: 11
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
        }
    }
}
