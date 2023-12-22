//import { he } from 'date-fns/locale';
import React from 'react';
import { Line } from 'react-chartjs-2'
import styles from 'styles/temperature_graph.module.css'

export default function TemperatureGraph() {
  return (
    <div className={styles.GraphContainer}>
      <div className={styles.Graph}>
        <Line
          data={{
            datasets: [
              {
                label: 'CPU temperature',
                borderColor: '#0091D5',
                backgroundColor: '#0091D5',
                data: [],
              },
              {
                label: 'Heater temperature',
                borderColor: '#EA6A47',
                backgroundColor: '#EA6A47',
                data: [],
              }
            ],
          }}
          options={{
            scales: {
              x: {
                type: 'realtime',
                realtime: {
                  duration: 1000 * 60 * 10,       // 10min
                  delay: -1000 * 60 * 3,
                  refresh: 1000 * 10,
                  pause: false,
                  onRefresh: refreshGraph
                }
              },
              y: {
                min: -10,
                max: 100,
              },
            },
          }}
        />
      </div>
    </div>
  )
}

async function refreshGraph(chart) {
  let temperature = await getTemperature();
  if( temperature == null ){
    return;
  }

  let [cpu_temp, heater_temp] = temperature;
  //console.log('cputemp:%d, heatertemp:%d', cpu_temp, heater_temp);
  
  let date = Date.now();
  chart.data.datasets[0].data.push({
    x: date,
    y: cpu_temp,
  });
  chart.data.datasets[1].data.push({
    x: date,
    y: heater_temp,
  });
}

async function getTemperature()
{
  const json = await fetch('http://192.168.24.107/details')
  .then( response => {
    if( response.ok ){
      return response.json();
    }
    else {
      return null;
    }
  })
  .catch(error => {
    console.error('通信に失敗しました', error);
  });

  if( json != null ){
    return [json.cpu_temp[0], json.heater_temp[0]];
  }

  return null;
}