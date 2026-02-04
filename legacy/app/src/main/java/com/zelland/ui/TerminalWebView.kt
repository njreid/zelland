package com.zelland.ui

import android.annotation.SuppressLint
import android.content.Context
import android.os.SystemClock
import android.text.InputType
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputConnectionWrapper
import android.webkit.WebView
import kotlin.math.abs

/**
 * Custom WebView optimized for terminal input
 */
class TerminalWebView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : WebView(context, attrs, defStyleAttr), GestureDetector.OnGestureListener {

    // Reference to the modifier state provider
    var modifierProvider: ModifierProvider? = null

    // Listener for swipes to switch Zellij tabs
    var onZellijTabSwipeListener: ((SwipeDirection) -> Unit)? = null

    interface ModifierProvider {
        fun getMetaState(): Int
        fun onModifierUsed()
    }

    enum class SwipeDirection { LEFT, RIGHT }

    private val gestureDetector = GestureDetector(context, this)
    
    // Swipe thresholds
    private val minSwipeDistance = 150
    private val minSwipeVelocity = 150
    // Margin for edge swipes (to allow ViewPager navigation later)
    private val edgeSwipeMargin = 80

    init {
        // Ensure the WebView is focusable to receive keyboard input
        isFocusable = true
        isFocusableInTouchMode = true
    }

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        return gestureDetector.onTouchEvent(event) || super.onTouchEvent(event)
    }

    // --- GestureDetector.OnGestureListener implementation ---

    override fun onDown(e: MotionEvent): Boolean = false
    override fun onShowPress(e: MotionEvent) {}
    override fun onSingleTapUp(e: MotionEvent): Boolean = false
    override fun onScroll(e1: MotionEvent?, e2: MotionEvent, distanceX: Float, distanceY: Float): Boolean = false
    override fun onLongPress(e: MotionEvent) {}

    override fun onFling(
        e1: MotionEvent?,
        e2: MotionEvent,
        velocityX: Float,
        velocityY: Float
    ): Boolean {
        if (e1 == null) return false
        
        val diffX = e2.x - e1.x
        val diffY = e2.y - e1.y
        
        // Detect horizontal swipe
        if (abs(diffX) > abs(diffY) && abs(diffX) > minSwipeDistance && abs(velocityX) > minSwipeVelocity) {
            // Check if it's an edge swipe (ignore those for Zellij tab switching)
            if (e1.x > edgeSwipeMargin && e1.x < width - edgeSwipeMargin) {
                if (diffX > 0) {
                    onZellijTabSwipeListener?.invoke(SwipeDirection.RIGHT)
                } else {
                    onZellijTabSwipeListener?.invoke(SwipeDirection.LEFT)
                }
                return true
            }
        }
        return false
    }

    // --- Input and Keyboard handling ---

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        val ic = super.onCreateInputConnection(outAttrs) ?: return null
        
        // If xterm.js has focus, it should provide an input connection.
        // We override the attributes to slim down the system keyboard.
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT or 
                             InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        
        // Some IMEs behave better if we don't use visible password but just disable suggestions
        outAttrs.imeOptions = EditorInfo.IME_ACTION_NONE or EditorInfo.IME_FLAG_NO_EXTRACT_UI
        
        return object : InputConnectionWrapper(ic, true) {
            override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
                val provider = modifierProvider
                if (provider != null) {
                    var metaState = provider.getMetaState()
                    // If modifiers are active, we try to convert the text to a key event
                    if (metaState != 0 && text != null && text.length == 1) {
                        val char = text[0]
                        
                        // If the character is uppercase, we should also include Shift in the meta state
                        if (char.isUpperCase()) {
                            metaState = metaState or KeyEvent.META_SHIFT_ON or KeyEvent.META_SHIFT_LEFT_ON
                        }
                        
                        val keyCode = getKeyCodeForChar(char)
                        if (keyCode != 0) {
                            val downTime = SystemClock.uptimeMillis()
                            dispatchKeyEvent(KeyEvent(downTime, downTime, KeyEvent.ACTION_DOWN, keyCode, 0, metaState))
                            dispatchKeyEvent(KeyEvent(downTime, downTime, KeyEvent.ACTION_UP, keyCode, 0, metaState))
                            provider.onModifierUsed()
                            return true
                        }
                    }
                }
                return super.commitText(text, newCursorPosition)
            }

            private fun getKeyCodeForChar(char: Char): Int {
                return when (char.lowercaseChar()) {
                    in 'a'..'z' -> KeyEvent.KEYCODE_A + (char.lowercaseChar() - 'a')
                    in '0'..'9' -> KeyEvent.KEYCODE_0 + (char - '0')
                    else -> 0
                }
            }
        }
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val provider = modifierProvider ?: return super.dispatchKeyEvent(event)
        
        val metaState = provider.getMetaState()
        
        // If there are active modifiers and this is a key down event,
        // we might need to recreate the event with the modifiers.
        if (metaState != 0 && (event.metaState and metaState) != metaState) {
            val modifiedEvent = KeyEvent(
                event.downTime,
                event.eventTime,
                event.action,
                event.keyCode,
                event.repeatCount,
                event.metaState or metaState,
                event.deviceId,
                event.scanCode,
                event.flags,
                event.source
            )
            val result = super.dispatchKeyEvent(modifiedEvent)
            if (event.action == KeyEvent.ACTION_UP) {
                provider.onModifierUsed()
            }
            return result
        }
        
        return super.dispatchKeyEvent(event)
    }
}
