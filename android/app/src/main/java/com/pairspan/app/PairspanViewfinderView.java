package com.pairspan.app;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.util.AttributeSet;
import com.journeyapps.barcodescanner.ViewfinderView;

public final class PairspanViewfinderView extends ViewfinderView {
    private final Paint border = new Paint(Paint.ANTI_ALIAS_FLAG);

    public PairspanViewfinderView(Context context, AttributeSet attrs) {
        super(context, attrs);
        border.setColor(0xFFFFFFFF);
        border.setStyle(Paint.Style.STROKE);
        border.setStrokeWidth(4 * getResources().getDisplayMetrics().density);
        border.setStrokeCap(Paint.Cap.ROUND);
    }

    @Override public void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        Rect frame = framingRect;
        if (frame == null) return;
        float segment = 24 * getResources().getDisplayMetrics().density;
        canvas.drawLine(frame.left, frame.top, frame.left + segment, frame.top, border);
        canvas.drawLine(frame.left, frame.top, frame.left, frame.top + segment, border);
        canvas.drawLine(frame.right - segment, frame.top, frame.right, frame.top, border);
        canvas.drawLine(frame.right, frame.top, frame.right, frame.top + segment, border);
        canvas.drawLine(frame.left, frame.bottom - segment, frame.left, frame.bottom, border);
        canvas.drawLine(frame.left, frame.bottom, frame.left + segment, frame.bottom, border);
        canvas.drawLine(frame.right - segment, frame.bottom, frame.right, frame.bottom, border);
        canvas.drawLine(frame.right, frame.bottom - segment, frame.right, frame.bottom, border);
    }
}
