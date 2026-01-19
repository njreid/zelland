package com.zelland.ui

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import com.zelland.R
import com.zelland.model.TerminalSession

class SessionAdapter(
    private var sessions: List<TerminalSession>,
    private val onConnect: (TerminalSession) -> Unit,
    private val onDisconnect: (TerminalSession) -> Unit
) : RecyclerView.Adapter<SessionAdapter.SessionViewHolder>() {

    fun updateSessions(newSessions: List<TerminalSession>) {
        sessions = newSessions
        notifyDataSetChanged()
    }

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): SessionViewHolder {
        val view = LayoutInflater.from(parent.context)
            .inflate(R.layout.item_session, parent, false)
        return SessionViewHolder(view)
    }

    override fun onBindViewHolder(holder: SessionViewHolder, position: Int) {
        holder.bind(sessions[position])
    }

    override fun getItemCount(): Int = sessions.size

    inner class SessionViewHolder(itemView: View) : RecyclerView.ViewHolder(itemView) {
        private val tvSessionTitle: TextView = itemView.findViewById(R.id.tvSessionTitle)
        private val tvHostInfo: TextView = itemView.findViewById(R.id.tvHostInfo)
        private val tvStatus: TextView = itemView.findViewById(R.id.tvStatus)
        private val btnConnect: Button = itemView.findViewById(R.id.btnConnect)
        private val btnDisconnect: Button = itemView.findViewById(R.id.btnDisconnect)

        fun bind(session: TerminalSession) {
            tvSessionTitle.text = session.title
            tvHostInfo.text = "${session.sshConfig.username}@${session.sshConfig.host}:${session.sshConfig.port}"

            // Update status and button visibility
            if (session.isConnected) {
                tvStatus.text = "Connected"
                tvStatus.setTextColor(itemView.context.getColor(R.color.status_connected))
                btnConnect.visibility = View.GONE
                btnDisconnect.visibility = View.VISIBLE
            } else {
                tvStatus.text = "Disconnected"
                tvStatus.setTextColor(itemView.context.getColor(R.color.status_disconnected))
                btnConnect.visibility = View.VISIBLE
                btnDisconnect.visibility = View.GONE
            }

            // Set click listeners
            btnConnect.setOnClickListener {
                onConnect(session)
            }

            btnDisconnect.setOnClickListener {
                onDisconnect(session)
            }
        }
    }
}
