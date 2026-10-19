#pragma once

#include <QQuickPaintedItem>
#include <QVariantList>

// Line graph of the last samples, drawn with QPainter so it renders with
// any Qt Quick backend, software included. `series` is a list of
// { values: [numbers], color }; samples fill the item from the right.
class SeriesPlot : public QQuickPaintedItem
{
    Q_OBJECT
    Q_PROPERTY(QVariantList series READ series WRITE setSeries NOTIFY seriesChanged)
    Q_PROPERTY(qreal maximum READ maximum WRITE setMaximum NOTIFY maximumChanged)
    Q_PROPERTY(qreal lineWidth READ lineWidth WRITE setLineWidth NOTIFY lineWidthChanged)
    Q_PROPERTY(int capacity READ capacity WRITE setCapacity NOTIFY capacityChanged)
    Q_PROPERTY(bool smooth READ smooth WRITE setSmooth NOTIFY smoothChanged)

public:
    explicit SeriesPlot(QQuickItem *parent = nullptr);

    QVariantList series() const { return m_series; }
    void setSeries(const QVariantList &series);
    qreal maximum() const { return m_maximum; }
    void setMaximum(qreal maximum);
    qreal lineWidth() const { return m_lineWidth; }
    void setLineWidth(qreal width);
    int capacity() const { return m_capacity; }
    void setCapacity(int capacity);
    bool smooth() const { return m_smooth; }
    void setSmooth(bool smooth);

    void paint(QPainter *painter) override;

Q_SIGNALS:
    void seriesChanged();
    void maximumChanged();
    void lineWidthChanged();
    void capacityChanged();
    void smoothChanged();

private:
    QVariantList m_series;
    qreal m_maximum = 100;
    qreal m_lineWidth = 1.5;
    int m_capacity = 60;
    bool m_smooth = false;
};
