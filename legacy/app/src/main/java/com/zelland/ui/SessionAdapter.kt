package com.zelland.ui

import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.ImageButton
import android.widget.TextView
import androidx.recyclerview.widget.RecyclerView
import com.zelland.R
import com.zelland.model.TerminalSession

class SessionAdapter(
    private var sessions: List<TerminalSession>,
    private val onConnect: (TerminalSession) -> Unit,
    private val onDelete: (TerminalSession) -> Unit
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
        private val btnConnect: Button = itemView.findViewById(R.id.btnConnect)
        private val btnDelete: ImageButton = itemView.findViewById(R.id.btnDelete)

        fun bind(session: TerminalSession) {
            tvSessionTitle.text = session.title

            // Set click listeners
            btnConnect.setOnClickListener {
                onConnect(session)
            }

            btnDelete.setOnClickListener {
                onDelete(session)
            }
        }
    }
}
