#include <QApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQuickStyle>

extern "C" int taskmgr_run(int argc, char **argv)
{
    static int s_argc = argc;
    QApplication app(s_argc, argv);
    app.setApplicationName(QStringLiteral("taskmgr"));
    app.setApplicationDisplayName(QStringLiteral("Gestionnaire des tâches"));
    app.setDesktopFileName(QStringLiteral("org.taskmgr"));
    app.setWindowIcon(QIcon::fromTheme(QStringLiteral("utilities-system-monitor")));
    QIcon::setFallbackThemeName(QStringLiteral("breeze"));

    // org.kde.desktop draws with QStyle, hence QApplication.
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) {
        QQuickStyle::setStyle(QStringLiteral("org.kde.desktop"));
    }

    QQmlApplicationEngine engine;
    QObject::connect(&engine, &QQmlApplicationEngine::objectCreationFailed, &app,
                     [] { QCoreApplication::exit(1); }, Qt::QueuedConnection);
    engine.loadFromModule("org.taskmgr", "Main");
    return app.exec();
}
