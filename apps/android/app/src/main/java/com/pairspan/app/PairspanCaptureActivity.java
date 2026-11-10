package com.pairspan.app;

import com.journeyapps.barcodescanner.CaptureActivity;
import com.journeyapps.barcodescanner.DecoratedBarcodeView;
import com.journeyapps.barcodescanner.Size;

public final class PairspanCaptureActivity extends CaptureActivity {
    @Override protected DecoratedBarcodeView initializeContent() {
        setContentView(R.layout.pairspan_capture);
        DecoratedBarcodeView scanner = findViewById(R.id.zxing_barcode_scanner);
        float density = getResources().getDisplayMetrics().density;
        int edge = Math.min((int) (272 * density), getResources().getDisplayMetrics().widthPixels - (int) (48 * density));
        scanner.getBarcodeView().setFramingRectSize(new Size(edge, edge));
        return scanner;
    }
}
