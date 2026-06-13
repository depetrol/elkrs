package elkrs;

import java.nio.file.Files;
import java.nio.file.Path;

import org.eclipse.elk.core.RecursiveGraphLayoutEngine;
import org.eclipse.elk.core.data.LayoutMetaDataService;
import org.eclipse.elk.core.util.BasicProgressMonitor;
import org.eclipse.elk.graph.ElkNode;
import org.eclipse.elk.graph.json.ElkGraphJson;

/**
 * Reads an ELK JSON graph from a file (or stdin with "-"), runs the recursive
 * graph layout engine, and prints the laid-out graph as JSON to stdout.
 * This is the golden-output generator for the Rust port.
 */
public final class Oracle {

    public static void main(String[] args) throws Exception {
        if (args.length < 1) {
            System.err.println("usage: oracle <graph.json | -> ");
            System.exit(2);
        }
        String input = args[0].equals("-")
                ? new String(System.in.readAllBytes())
                : Files.readString(Path.of(args[0]));

        // Register all bundled algorithms (no Eclipse extension registry here).
        LayoutMetaDataService.getInstance().registerLayoutMetaDataProviders(
                new org.eclipse.elk.alg.layered.options.LayeredMetaDataProvider(),
                new org.eclipse.elk.alg.force.options.ForceMetaDataProvider(),
                new org.eclipse.elk.alg.force.options.StressMetaDataProvider(),
                new org.eclipse.elk.alg.mrtree.options.MrTreeMetaDataProvider(),
                new org.eclipse.elk.alg.radial.options.RadialMetaDataProvider(),
                new org.eclipse.elk.alg.rectpacking.options.RectPackingMetaDataProvider(),
                new org.eclipse.elk.alg.spore.options.SporeMetaDataProvider(),
                new org.eclipse.elk.alg.disco.options.DisCoMetaDataProvider(),
                new org.eclipse.elk.alg.topdownpacking.options.TopdownpackingMetaDataProvider());

        ElkNode graph = ElkGraphJson.forGraph(input).toElk();
        new RecursiveGraphLayoutEngine().layout(graph, new BasicProgressMonitor());

        String out = ElkGraphJson.forGraph(graph)
                .omitZeroPositions(false)
                .omitZeroDimension(false)
                .shortLayoutOptionKeys(false)
                .prettyPrint(true)
                .toJson();
        System.out.println(out);
    }

    private Oracle() { }
}
