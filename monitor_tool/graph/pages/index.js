import Layout from "@/components/layout"
import TemperatureGraph from "@/components/temperature_graph"

export default function Home() {
  return (
    <Layout>
      <h1>Regin Heater Temperature Graph</h1>
      <TemperatureGraph></TemperatureGraph>
    </Layout>
  )
}
