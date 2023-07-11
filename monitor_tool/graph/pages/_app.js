import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  TimeScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
} from "chart.js";
import 'import/chartjs-adapter-luxon/index.js'
import StreamingPlugin from 'chartjs-plugin-streaming'

ChartJS.register(
  CategoryScale,
  LinearScale,
  TimeScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  StreamingPlugin
);

export default function MyApp({ Component, pageProps }) {
  return (
    <Component {...pageProps} />
  )
}

