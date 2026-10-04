# Interface JavaScript appelée depuis la page (noms conservés).
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}
