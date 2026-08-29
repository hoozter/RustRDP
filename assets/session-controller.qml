import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.kde.layershell 1.0 as LayerShell

Window {
    id: root

    readonly property string controlUrl: Qt.application.arguments.length > 2
        ? Qt.application.arguments[Qt.application.arguments.length - 2] : ""
    readonly property string profileName: Qt.application.arguments.length > 1
        ? Qt.application.arguments[Qt.application.arguments.length - 1] : "Remote desktop"

    width: 390
    height: 48
    visible: true
    color: "transparent"
    flags: Qt.FramelessWindowHint
    title: profileName + " — RustRDP controls"

    LayerShell.Window.anchors: LayerShell.Window.AnchorTop
    LayerShell.Window.layer: LayerShell.Window.LayerOverlay
    LayerShell.Window.exclusionZone: 0
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    LayerShell.Window.scope: "rustrdp-session-controller"

    function sendCommand(command) {
        if (controlUrl.length === 0)
            return
        const request = new XMLHttpRequest()
        request.open("POST", controlUrl + "/" + command)
        request.send("")
    }

    // A loaded QML Window is hidden by default. Report readiness only after
    // this surface has been made visible, otherwise RustRDP could launch a
    // fullscreen session behind a controller that was alive but never mapped.
    Timer {
        interval: 150
        running: root.visible
        repeat: false
        onTriggered: root.sendCommand("ready")
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 4
        radius: 10
        color: "#ee20242b"
        border.width: 1
        border.color: "#59636f"

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 7
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: root.profileName
                color: "#f2f4f7"
                font.pixelSize: 14
                font.bold: true
                elide: Text.ElideRight
            }

            Button {
                text: "Minimize"
                focusPolicy: Qt.NoFocus
                onClicked: root.sendCommand("minimize")
            }

            Button {
                text: "Disconnect"
                focusPolicy: Qt.NoFocus
                onClicked: root.sendCommand("disconnect")
            }
        }
    }
}
