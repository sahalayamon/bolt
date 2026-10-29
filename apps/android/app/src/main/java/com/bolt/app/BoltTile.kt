package com.bolt.app

import android.content.Intent
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/** Quick-settings tile: tap to turn Bolt on or off from the notification shade. */
class BoltTile : TileService() {
    override fun onStartListening() = render()

    override fun onClick() {
        if (Engine.isRunning()) startService(Intent(this, TransferService::class.java).setAction(TransferService.ACTION_STOP))
        else TransferService.start(this)
        // the service refreshes the tile once it has actually started or stopped
    }

    private fun render() {
        val tile = qsTile ?: return
        val on = Engine.isRunning()
        tile.state = if (on) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = "Bolt"
        tile.subtitle = if (on) "On" else "Off"
        tile.updateTile()
    }
}
