#include "seriesplot.h"

#include <QPainter>
#include <QPainterPath>

SeriesPlot::SeriesPlot(QQuickItem *parent)
    : QQuickPaintedItem(parent)
{
    setAntialiasing(true);
}

void SeriesPlot::setSeries(const QVariantList &series)
{
    m_series = series;
    Q_EMIT seriesChanged();
    update();
}

void SeriesPlot::setMaximum(qreal maximum)
{
    if (qFuzzyCompare(m_maximum, maximum)) {
        return;
    }
    m_maximum = maximum;
    Q_EMIT maximumChanged();
    update();
}

void SeriesPlot::setLineWidth(qreal width)
{
    if (qFuzzyCompare(m_lineWidth, width)) {
        return;
    }
    m_lineWidth = width;
    Q_EMIT lineWidthChanged();
    update();
}

void SeriesPlot::setCapacity(int capacity)
{
    if (m_capacity == capacity || capacity < 2) {
        return;
    }
    m_capacity = capacity;
    Q_EMIT capacityChanged();
    update();
}

void SeriesPlot::setSmooth(bool smooth)
{
    if (m_smooth == smooth) {
        return;
    }
    m_smooth = smooth;
    Q_EMIT smoothChanged();
    update();
}

void SeriesPlot::paint(QPainter *painter)
{
    const qreal w = width();
    const qreal h = height();
    const qreal top = m_maximum > 0 ? m_maximum : 1;
    const qreal step = w / (m_capacity - 1);
    painter->setRenderHint(QPainter::Antialiasing);
    for (const QVariant &entry : m_series) {
        const QVariantMap s = entry.toMap();
        const QVariantList values = s.value(QStringLiteral("values")).toList();
        if (values.size() < 2) {
            continue;
        }
        const QColor color = s.value(QStringLiteral("color")).value<QColor>();
        QPainterPath line;
        const qreal x0 = w - (values.size() - 1) * step;
        QPointF prev;
        for (int i = 0; i < values.size(); ++i) {
            const qreal v = qBound(0.0, values[i].toDouble() / top, 1.0);
            const QPointF p(x0 + i * step, h - v * h);
            if (i == 0) {
                line.moveTo(p);
            } else if (m_smooth) {
                // Horizontal tangents at each sample: no overshoot.
                const qreal mid = (prev.x() + p.x()) / 2;
                line.cubicTo(QPointF(mid, prev.y()), QPointF(mid, p.y()), p);
            } else {
                line.lineTo(p);
            }
            prev = p;
        }
        QPainterPath area = line;
        area.lineTo(w, h);
        area.lineTo(x0, h);
        area.closeSubpath();
        QColor fill = color;
        fill.setAlphaF(0.15);
        painter->fillPath(area, fill);
        painter->strokePath(line, QPen(color, m_lineWidth));
    }
}
